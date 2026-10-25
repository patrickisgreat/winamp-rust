//! Symphonia-backed decoding of local files and network streams.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
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
    pub fn from_str(s: &str) -> Source {
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            Source::Url(s.to_string())
        } else {
            Source::File(PathBuf::from(s))
        }
    }

    pub fn as_display(&self) -> String {
        match self {
            Source::File(p) => p.to_string_lossy().to_string(),
            Source::Url(u) => u.clone(),
        }
    }

    pub fn is_url(&self) -> bool {
        matches!(self, Source::Url(_))
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Source::File(p) => Some(p),
            Source::Url(_) => None,
        }
    }
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

pub struct OpenedTrack {
    pub info: TrackInfo,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_buf: Option<SampleBuffer<f32>>,
    pub title_slot: Option<TitleSlot>,
    channels: usize,
    pub eof: bool,
}

fn codec_name(c: symphonia::core::codecs::CodecType) -> String {
    use symphonia::core::codecs::*;
    let name = match c {
        CODEC_TYPE_MP3 => "MPEG Layer 3",
        CODEC_TYPE_MP2 => "MPEG Layer 2",
        CODEC_TYPE_MP1 => "MPEG Layer 1",
        CODEC_TYPE_AAC => "AAC",
        CODEC_TYPE_FLAC => "FLAC",
        CODEC_TYPE_VORBIS => "Ogg Vorbis",
        CODEC_TYPE_OPUS => "Opus",
        CODEC_TYPE_ALAC => "Apple Lossless",
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24LE | CODEC_TYPE_PCM_S24BE | CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE | CODEC_TYPE_PCM_U8 | CODEC_TYPE_PCM_S8 | CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE | CODEC_TYPE_PCM_F64LE | CODEC_TYPE_PCM_F64BE => "PCM",
        CODEC_TYPE_ADPCM_IMA_WAV | CODEC_TYPE_ADPCM_MS => "ADPCM",
        _ => return format!("{c:?}"),
    };
    name.to_string()
}

/// File extensions we claim to support.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "mp1", "ogg", "oga", "flac", "wav", "wave", "aif", "aiff", "aifc", "m4a", "mp4", "aac",
    "adts", "mkv", "mka", "webm", "caf", "alac",
];

pub fn is_supported_path(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_ascii_lowercase();
            SUPPORTED_EXTENSIONS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

impl OpenedTrack {
    pub fn open(source: &Source) -> Result<Self> {
        match source {
            Source::File(p) => Self::open_file(p),
            Source::Url(u) => Self::open_url(u),
        }
    }

    fn open_file(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut hint = Hint::new();
        if let Some(ext) = path.extension() {
            hint.with_extension(&ext.to_string_lossy());
        }
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let meta = crate::tags::read_meta(path);
        let mut t = Self::from_mss(mss, hint, true)?;
        t.info.meta = meta.clone();
        if t.info.duration.is_none() {
            t.info.duration = meta.duration;
        }
        if t.info.bitrate_kbps.is_none() {
            t.info.bitrate_kbps = meta.bitrate_kbps.or_else(|| {
                match (meta.file_size, t.info.duration) {
                    (Some(size), Some(d)) if d.as_secs_f64() > 0.0 => {
                        Some((size as f64 * 8.0 / d.as_secs_f64() / 1000.0).round() as u32)
                    }
                    _ => None,
                }
            });
        }
        t.info.title = Some(meta.display_title(path));
        Ok(t)
    }

    fn open_url(url: &str) -> Result<Self> {
        let (reader, sinfo, title) = stream::open(url)?;
        let mut hint = Hint::new();
        if let Some(ext) = sinfo.content_type.as_deref().and_then(stream::extension_for_mime) {
            hint.with_extension(ext);
        } else if let Some(ext) = url.rsplit('.').next().filter(|e| e.len() <= 4) {
            hint.with_extension(ext);
        }
        let reader: Box<dyn Read + Send + Sync> = reader;
        let mss = MediaSourceStream::new(Box::new(ReadOnlySource::new(reader)), Default::default());
        let mut t = Self::from_mss(mss, hint, false)?;
        t.info.bitrate_kbps = sinfo.bitrate_kbps.or(t.info.bitrate_kbps);
        t.info.title = sinfo.name.clone().or_else(|| Some(url.to_string()));
        t.info.stream = Some(sinfo);
        t.title_slot = Some(title);
        Ok(t)
    }

    fn from_mss(mss: MediaSourceStream, hint: Hint, seekable: bool) -> Result<Self> {
        let fmt_opts = FormatOptions { enable_gapless: true, ..Default::default() };
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &MetadataOptions::default())
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
            .map(|n| Duration::from_secs_f64(n as f64 / sample_rate as f64))
            .or_else(|| {
                params.time_base.zip(params.n_frames).map(|(tb, n)| {
                    let t = tb.calc_time(n);
                    Duration::from_secs_f64(t.seconds as f64 + t.frac)
                })
            });
        let bitrate = params.bits_per_sample.map(|b| b * sample_rate * channels as u32 / 1000);
        let codec = codec_name(params.codec);
        let info = TrackInfo {
            sample_rate,
            channels: channels as u16,
            bitrate_kbps: bitrate,
            duration,
            seekable: seekable && duration.is_some(),
            codec,
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
            eof: false,
        })
    }

    /// Decode the next packet into interleaved stereo f32 samples appended to `out`.
    /// Returns Ok(false) on end of stream.
    pub fn decode_next(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        if self.eof {
            return Ok(false);
        }
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.eof = true;
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
                    let need = frames as u64;
                    let buf = match &mut self.sample_buf {
                        Some(b) if b.capacity() >= frames * spec.channels.count() => b,
                        _ => {
                            self.sample_buf = Some(SampleBuffer::<f32>::new(need, spec));
                            self.sample_buf.as_mut().unwrap()
                        }
                    };
                    buf.copy_interleaved_ref(audio);
                    let samples = buf.samples();
                    let ch = spec.channels.count().max(1);
                    self.channels = ch;
                    match ch {
                        1 => {
                            for s in samples {
                                out.push(*s);
                                out.push(*s);
                            }
                        }
                        2 => out.extend_from_slice(samples),
                        _ => {
                            for frame in samples.chunks_exact(ch) {
                                // Simple downmix: average everything beyond L/R into both.
                                let extra: f32 = frame[2..].iter().sum::<f32>() / (ch as f32);
                                out.push((frame[0] + extra).clamp(-1.0, 1.0));
                                out.push((frame[1] + extra).clamp(-1.0, 1.0));
                            }
                        }
                    }
                    return Ok(true);
                }
                Err(SymError::DecodeError(e)) => {
                    log::debug!("decode error (skipping packet): {e}");
                    continue;
                }
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.eof = true;
                    return Ok(false);
                }
                Err(e) => return Err(anyhow!("decode error: {e}")),
            }
        }
    }

    pub fn seek(&mut self, secs: f64) -> Result<f64> {
        if !self.info.seekable {
            return Err(anyhow!("stream is not seekable"));
        }
        let secs = secs.max(0.0);
        let to = SeekTo::Time { time: Time::from(secs), track_id: Some(self.track_id) };
        let seeked = self
            .format
            .seek(SeekMode::Accurate, to)
            .or_else(|_| self.format.seek(SeekMode::Coarse, SeekTo::Time { time: Time::from(secs), track_id: Some(self.track_id) }))
            .map_err(|e| anyhow!("seek failed: {e}"))?;
        self.decoder.reset();
        self.eof = false;
        let actual = self
            .format
            .tracks()
            .iter()
            .find(|t| t.id == self.track_id)
            .and_then(|t| t.codec_params.time_base)
            .map(|tb| {
                let t = tb.calc_time(seeked.actual_ts);
                t.seconds as f64 + t.frac
            })
            .unwrap_or(secs);
        Ok(actual)
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
}
