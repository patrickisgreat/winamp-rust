//! Symphonia-backed decoding of local files and network streams.

use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecType, Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::stream::{self, StreamInfo, TitleSlot};
use crate::tags::TrackMeta;

/// Something that can be played.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    File(PathBuf),
    Url(String),
}

impl Source {
    /// Classify user input: anything starting with `http://` or `https://`
    /// is a URL, everything else is a local path.
    pub fn parse(text: &str) -> Source {
        if is_http_url(text) {
            Source::Url(text.to_string())
        } else {
            Source::File(PathBuf::from(text))
        }
    }

    pub fn is_url(&self) -> bool {
        matches!(self, Source::Url(_))
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Source::File(path) => Some(path),
            Source::Url(_) => None,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::File(path) => write!(f, "{}", path.display()),
            Source::Url(url) => f.write_str(url),
        }
    }
}

impl From<&str> for Source {
    fn from(text: &str) -> Self {
        Source::parse(text)
    }
}

/// True for `http://` and `https://` addresses, case-insensitively.
pub fn is_http_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Static description of an opened track.
#[derive(Clone, Debug, Default)]
pub struct TrackInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate_kbps: Option<u32>,
    pub duration: Option<Duration>,
    pub seekable: bool,
    pub codec: String,
    /// Stream name / tag-derived title if known.
    pub title: Option<String>,
    pub meta: TrackMeta,
    pub stream: Option<StreamInfo>,
}

/// A source that has been probed and has a decoder ready.
pub struct OpenedTrack {
    pub info: TrackInfo,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_buf: Option<SampleBuffer<f32>>,
    /// Receives ICY "now playing" titles for network streams.
    pub title_slot: Option<TitleSlot>,
    channels: usize,
    finished: bool,
}

fn codec_name(codec: CodecType) -> String {
    use symphonia::core::codecs::*;
    let name = match codec {
        CODEC_TYPE_MP3 => "MPEG Layer 3",
        CODEC_TYPE_MP2 => "MPEG Layer 2",
        CODEC_TYPE_MP1 => "MPEG Layer 1",
        CODEC_TYPE_AAC => "AAC",
        CODEC_TYPE_FLAC => "FLAC",
        CODEC_TYPE_VORBIS => "Ogg Vorbis",
        CODEC_TYPE_OPUS => "Opus",
        CODEC_TYPE_ALAC => "Apple Lossless",
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24LE
        | CODEC_TYPE_PCM_S24BE | CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE
        | CODEC_TYPE_PCM_U8 | CODEC_TYPE_PCM_S8 | CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE
        | CODEC_TYPE_PCM_F64LE | CODEC_TYPE_PCM_F64BE => "PCM",
        CODEC_TYPE_ADPCM_IMA_WAV | CODEC_TYPE_ADPCM_MS => "ADPCM",
        _ => return format!("{codec:?}"),
    };
    name.to_string()
}

/// File extensions we claim to support.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "mp1", "ogg", "oga", "flac", "wav", "wave", "aif", "aiff", "aifc", "m4a", "mp4",
    "aac", "adts", "mkv", "mka", "webm", "caf", "alac",
];

pub fn is_supported_path(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
}

impl OpenedTrack {
    pub fn open(source: &Source) -> Result<Self> {
        match source {
            Source::File(path) => Self::open_file(path),
            Source::Url(url) => Self::open_url(url),
        }
    }

    fn open_file(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut hint = Hint::new();
        if let Some(ext) = path.extension() {
            hint.with_extension(&ext.to_string_lossy());
        }
        let media = MediaSourceStream::new(Box::new(file), Default::default());
        let meta = crate::tags::read_meta(path);
        let mut track = Self::from_media(media, hint, true)?;
        track.info.duration = track.info.duration.or(meta.duration);
        track.info.bitrate_kbps = track
            .info
            .bitrate_kbps
            .or(meta.bitrate_kbps)
            .or_else(|| average_bitrate_kbps(meta.file_size, track.info.duration));
        track.info.title = Some(meta.display_title(path));
        track.info.meta = meta;
        Ok(track)
    }

    fn open_url(url: &str) -> Result<Self> {
        let opened = stream::open(url)?;
        let mut hint = Hint::new();
        let mime_ext = opened
            .info
            .content_type
            .as_deref()
            .and_then(stream::extension_for_mime);
        let url_ext = url.rsplit('.').next().filter(|ext| ext.len() <= 4);
        if let Some(ext) = mime_ext.or(url_ext) {
            hint.with_extension(ext);
        }
        let media = MediaSourceStream::new(
            Box::new(ReadOnlySource::new(opened.reader)),
            Default::default(),
        );
        let mut track = Self::from_media(media, hint, false)?;
        track.info.bitrate_kbps = opened.info.bitrate_kbps.or(track.info.bitrate_kbps);
        track.info.title = Some(opened.info.name.clone().unwrap_or_else(|| url.to_string()));
        track.info.stream = Some(opened.info);
        track.title_slot = Some(opened.title);
        Ok(track)
    }

    fn from_media(media: MediaSourceStream, hint: Hint, seekable: bool) -> Result<Self> {
        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let probed = symphonia::default::get_probe()
            .format(&hint, media, &format_opts, &MetadataOptions::default())
            .map_err(|e| anyhow!("unsupported or corrupt file: {e}"))?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow!("no playable audio track"))?;
        let params = track.codec_params.clone();
        let track_id = track.id;
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| anyhow!("no decoder: {e}"))?;
        let sample_rate = params.sample_rate.unwrap_or(44100);
        let channels = params.channels.map(|c| c.count()).unwrap_or(2).max(1);
        let duration = params
            .n_frames
            .map(|frames| Duration::from_secs_f64(frames as f64 / f64::from(sample_rate)))
            .or_else(|| {
                params
                    .time_base
                    .zip(params.n_frames)
                    .map(|(time_base, frames)| {
                        let time = time_base.calc_time(frames);
                        Duration::from_secs_f64(time.seconds as f64 + time.frac)
                    })
            });
        let bitrate_kbps = params
            .bits_per_sample
            .map(|bits| bits * sample_rate * channels as u32 / 1000);
        let info = TrackInfo {
            sample_rate,
            channels: channels as u16,
            bitrate_kbps,
            duration,
            seekable: seekable && duration.is_some(),
            codec: codec_name(params.codec),
            title: None,
            meta: TrackMeta::default(),
            stream: None,
        };
        Ok(Self {
            info,
            format,
            decoder,
            track_id,
            sample_buf: None,
            title_slot: None,
            channels,
            finished: false,
        })
    }

    /// True once the end of the stream has been reached (until the next seek).
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Decode the next packet into interleaved stereo f32 samples appended to `out`.
    /// Returns Ok(false) on end of stream.
    pub fn decode_next(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        if self.finished {
            return Ok(false);
        }
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.finished = true;
                    return Ok(false);
                }
                Err(SymError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(anyhow!("read error: {e}")),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(audio) => {
                    let spec = *audio.spec();
                    let frames = audio.frames();
                    if frames == 0 {
                        continue;
                    }
                    let channels = spec.channels.count().max(1);
                    let buffer = match &mut self.sample_buf {
                        Some(buffer) if buffer.capacity() >= frames * channels => buffer,
                        _ => self
                            .sample_buf
                            .insert(SampleBuffer::<f32>::new(frames as u64, spec)),
                    };
                    buffer.copy_interleaved_ref(audio);
                    self.channels = channels;
                    downmix_to_stereo(buffer.samples(), channels, out);
                    return Ok(true);
                }
                Err(SymError::DecodeError(e)) => {
                    log::debug!("decode error (skipping packet): {e}");
                    continue;
                }
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.finished = true;
                    return Ok(false);
                }
                Err(e) => return Err(anyhow!("decode error: {e}")),
            }
        }
    }

    /// Seek to `secs`. Returns the position actually reached.
    pub fn seek(&mut self, secs: f64) -> Result<f64> {
        if !self.info.seekable {
            return Err(anyhow!("stream is not seekable"));
        }
        let secs = secs.max(0.0);
        let target = || SeekTo::Time {
            time: Time::from(secs),
            track_id: Some(self.track_id),
        };
        let seeked = self
            .format
            .seek(SeekMode::Accurate, target())
            .or_else(|_| self.format.seek(SeekMode::Coarse, target()))
            .map_err(|e| anyhow!("seek failed: {e}"))?;
        self.decoder.reset();
        self.finished = false;
        let actual = self
            .format
            .tracks()
            .iter()
            .find(|t| t.id == self.track_id)
            .and_then(|t| t.codec_params.time_base)
            .map(|time_base| {
                let time = time_base.calc_time(seeked.actual_ts);
                time.seconds as f64 + time.frac
            })
            .unwrap_or(secs);
        Ok(actual)
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// Average bitrate from file size and duration, for formats whose headers
/// do not state one.
fn average_bitrate_kbps(file_size: Option<u64>, duration: Option<Duration>) -> Option<u32> {
    match (file_size, duration) {
        (Some(size), Some(duration)) if duration.as_secs_f64() > 0.0 => {
            Some((size as f64 * 8.0 / duration.as_secs_f64() / 1000.0).round() as u32)
        }
        _ => None,
    }
}

/// Append `samples` (interleaved, `channels` wide) to `out` as stereo frames.
fn downmix_to_stereo(samples: &[f32], channels: usize, out: &mut Vec<f32>) {
    match channels {
        1 => out.extend(samples.iter().flat_map(|&s| [s, s])),
        2 => out.extend_from_slice(samples),
        _ => {
            for frame in samples.chunks_exact(channels) {
                // Simple downmix: average everything beyond L/R into both.
                let extra: f32 = frame[2..].iter().sum::<f32>() / channels as f32;
                out.push((frame[0] + extra).clamp(-1.0, 1.0));
                out.push((frame[1] + extra).clamp(-1.0, 1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_urls_and_paths() {
        assert_eq!(
            Source::parse("HTTP://example.com/a"),
            Source::Url("HTTP://example.com/a".into())
        );
        assert_eq!(
            Source::parse("https://x/y.pls"),
            Source::Url("https://x/y.pls".into())
        );
        assert_eq!(
            Source::parse("/music/a.mp3"),
            Source::File("/music/a.mp3".into())
        );
        assert_eq!(
            Source::parse("ftp://host/a.mp3"),
            Source::File("ftp://host/a.mp3".into())
        );
        assert_eq!(
            Source::from("relative.mp3"),
            Source::File("relative.mp3".into())
        );
    }

    #[test]
    fn display_shows_path_or_url() {
        assert_eq!(Source::File("/a/b.mp3".into()).to_string(), "/a/b.mp3");
        assert_eq!(Source::Url("http://x".into()).to_string(), "http://x");
        assert_eq!(Source::File("/a".into()).path(), Some(Path::new("/a")));
        assert_eq!(Source::Url("http://x".into()).path(), None);
    }

    #[test]
    fn supported_extensions_are_case_insensitive() {
        assert!(is_supported_path("song.MP3"));
        assert!(is_supported_path("/x/y/track.flac"));
        assert!(!is_supported_path("notes.txt"));
        assert!(!is_supported_path("no_extension"));
    }

    #[test]
    fn downmix_handles_mono_stereo_and_surround() {
        let mut out = Vec::new();
        downmix_to_stereo(&[0.5, -0.5], 1, &mut out);
        assert_eq!(out, [0.5, 0.5, -0.5, -0.5]);
        out.clear();
        downmix_to_stereo(&[0.1, 0.2], 2, &mut out);
        assert_eq!(out, [0.1, 0.2]);
        out.clear();
        downmix_to_stereo(&[1.0, 0.0, 0.3, 0.3], 4, &mut out);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 1.0).abs() < 1e-6, "clamped to 1.0");
        assert!((out[1] - 0.15).abs() < 1e-6);
    }

    #[test]
    fn average_bitrate_needs_both_inputs() {
        assert_eq!(
            average_bitrate_kbps(Some(1_000_000), Some(Duration::from_secs(50))),
            Some(160)
        );
        assert_eq!(
            average_bitrate_kbps(None, Some(Duration::from_secs(50))),
            None
        );
        assert_eq!(average_bitrate_kbps(Some(1), Some(Duration::ZERO)), None);
    }

    /// Write a 16-bit PCM stereo WAV of `frames` frames at 8 kHz.
    fn write_test_wav(name: &str, frames: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winamp-audio-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let sample_rate: u32 = 8000;
        let channels: u16 = 2;
        let data_len = frames * u32::from(channels) * 2;
        let mut file = File::create(&path).unwrap();
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data_len).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
        file.write_all(&channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&(sample_rate * u32::from(channels) * 2).to_le_bytes())
            .unwrap();
        file.write_all(&(channels * 2).to_le_bytes()).unwrap();
        file.write_all(&16u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_len.to_le_bytes()).unwrap();
        for i in 0..frames {
            let sample = ((i % 100) as i16 - 50) * 100;
            file.write_all(&sample.to_le_bytes()).unwrap();
            file.write_all(&sample.to_le_bytes()).unwrap();
        }
        path
    }

    #[test]
    fn opens_decodes_and_seeks_a_wav_file() {
        let path = write_test_wav("two_seconds.wav", 16_000);
        let mut track = OpenedTrack::open(&Source::File(path.clone())).unwrap();
        assert_eq!(track.info.sample_rate, 8000);
        assert_eq!(track.info.channels, 2);
        assert_eq!(track.info.codec, "PCM");
        assert!(track.info.seekable);
        assert_eq!(track.info.duration, Some(Duration::from_secs(2)));
        assert_eq!(track.info.title.as_deref(), Some("two_seconds"));

        let mut all = Vec::new();
        let mut chunk = Vec::new();
        while track.decode_next(&mut chunk).unwrap() {
            all.extend_from_slice(&chunk);
            chunk.clear();
        }
        assert!(track.is_finished());
        assert_eq!(all.len(), 16_000 * 2);
        assert!(!track.decode_next(&mut chunk).unwrap(), "stays finished");

        // WAV seeks land on a packet boundary, so allow some slack but insist
        // that the reported position matches what is left to decode.
        let reached = track.seek(1.0).unwrap();
        assert!((reached - 1.0).abs() < 0.25, "seeked to {reached}");
        assert!(!track.is_finished());
        let mut rest = Vec::new();
        while track.decode_next(&mut chunk).unwrap() {
            rest.extend_from_slice(&chunk);
            chunk.clear();
        }
        let expected_frames = ((2.0 - reached) * 8000.0).round() as i64;
        let got_frames = (rest.len() / 2) as i64;
        assert!(
            (got_frames - expected_frames).abs() <= 2,
            "expected {expected_frames} frames after seek, got {got_frames}"
        );
    }

    #[test]
    fn open_reports_missing_and_garbage_files_as_errors() {
        let missing = OpenedTrack::open(&Source::File("/no/such/file.wav".into()));
        assert!(missing.is_err());
        let dir = std::env::temp_dir().join(format!("winamp-audio-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let garbage = dir.join("garbage.mp3");
        std::fs::write(&garbage, b"this is not audio at all").unwrap();
        let Err(err) = OpenedTrack::open(&Source::File(garbage)) else {
            panic!("garbage should not decode");
        };
        assert!(err.to_string().contains("unsupported or corrupt"), "{err}");
    }
}
