//! HTTP streaming (Shoutcast / Icecast) with ICY metadata extraction.

use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = "WinampRust/0.1 (like Winamp/2.95)";
/// ICY metadata block lengths are sent as a byte count divided by 16.
const META_LENGTH_UNIT: usize = 16;

/// Information learned from the HTTP response headers of a stream.
#[derive(Clone, Debug, Default)]
pub struct StreamInfo {
    pub name: Option<String>,
    pub genre: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub content_type: Option<String>,
    /// Bytes of audio between ICY metadata blocks, when the server sends them.
    pub metaint: Option<usize>,
}

/// Mailbox that receives "StreamTitle" updates from the ICY metadata. The
/// reader writes a title in, the engine takes it out.
pub type TitleSlot = Arc<Mutex<Option<String>>>;

/// A reader that strips ICY metadata blocks out of a Shoutcast stream.
pub struct IcyReader<R: Read> {
    inner: R,
    metaint: usize,
    /// Audio bytes left before the next metadata block.
    until_meta: usize,
    title: TitleSlot,
}

impl<R: Read> IcyReader<R> {
    pub fn new(inner: R, metaint: usize, title: TitleSlot) -> Self {
        Self {
            inner,
            metaint,
            until_meta: metaint,
            title,
        }
    }

    /// Consume one metadata block. Returns Ok(false) at end of stream.
    fn read_meta_block(&mut self) -> io::Result<bool> {
        let mut length_byte = [0u8; 1];
        if self.inner.read(&mut length_byte)? == 0 {
            return Ok(false);
        }
        let length = usize::from(length_byte[0]) * META_LENGTH_UNIT;
        if length == 0 {
            return Ok(true);
        }
        let mut block = vec![0u8; length];
        self.inner.read_exact(&mut block)?;
        if let Some(title) = parse_stream_title(&String::from_utf8_lossy(&block)) {
            if let Ok(mut slot) = self.title.lock() {
                *slot = Some(title);
            }
        }
        Ok(true)
    }
}

impl<R: Read> Read for IcyReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.metaint == 0 {
            return self.inner.read(buf);
        }
        if self.until_meta == 0 {
            if !self.read_meta_block()? {
                return Ok(0);
            }
            self.until_meta = self.metaint;
        }
        let want = buf.len().min(self.until_meta);
        let got = self.inner.read(&mut buf[..want])?;
        self.until_meta -= got;
        Ok(got)
    }
}

/// Extract the `StreamTitle='...';` value from an ICY metadata block.
pub fn parse_stream_title(meta: &str) -> Option<String> {
    const KEY: &str = "StreamTitle='";
    let start = meta.find(KEY)? + KEY.len();
    let rest = &meta[start..];
    let end = rest.find("';").unwrap_or(rest.len());
    let title = rest[..end].trim();
    (!title.is_empty()).then(|| title.to_string())
}

/// An opened HTTP(S) audio stream.
pub struct OpenedStream {
    /// Audio bytes with any ICY metadata already stripped out.
    pub reader: Box<dyn Read + Send + Sync>,
    pub info: StreamInfo,
    pub title: TitleSlot,
}

/// Open an HTTP(S) audio stream, asking the server for ICY metadata.
pub fn open(url: &str) -> Result<OpenedStream> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .user_agent(USER_AGENT)
        .build();
    let response = agent
        .get(url)
        .set("Icy-MetaData", "1")
        .call()
        .with_context(|| format!("opening {url}"))?;
    if response.status() >= 400 {
        return Err(anyhow!("HTTP {} for {url}", response.status()));
    }
    let header = |name: &str| response.header(name).map(str::to_string);
    let info = StreamInfo {
        name: header("icy-name"),
        genre: header("icy-genre"),
        bitrate_kbps: response
            .header("icy-br")
            .and_then(|s| s.trim().parse().ok()),
        content_type: response.header("content-type").map(str::to_lowercase),
        metaint: response
            .header("icy-metaint")
            .and_then(|s| s.trim().parse().ok()),
    };
    let title: TitleSlot = Arc::new(Mutex::new(None));
    let body = response.into_reader();
    let reader: Box<dyn Read + Send + Sync> = match info.metaint {
        Some(metaint) if metaint > 0 => Box::new(IcyReader::new(body, metaint, Arc::clone(&title))),
        _ => Box::new(body),
    };
    Ok(OpenedStream {
        reader,
        info,
        title,
    })
}

/// Guess a container/codec extension hint from a MIME type.
pub fn extension_for_mime(mime: &str) -> Option<&'static str> {
    let media_type = mime.split(';').next().unwrap_or("").trim();
    Some(match media_type {
        "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" => "mp3",
        "audio/aac" | "audio/aacp" | "audio/x-aac" => "aac",
        "audio/ogg" | "application/ogg" | "audio/vorbis" => "ogg",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mp4" | "audio/x-m4a" | "video/mp4" => "m4a",
        _ => return None,
    })
}

/// Fetch a small text resource (used for remote .pls / .m3u playlists).
pub fn fetch_text(url: &str, max_bytes: usize) -> Result<String> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("fetching {url}"))?;
    let mut text = String::new();
    response
        .into_reader()
        .take(max_bytes as u64)
        .read_to_string(&mut text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_block(text: &str) -> Vec<u8> {
        let mut block = text.as_bytes().to_vec();
        let padded = block.len().div_ceil(META_LENGTH_UNIT) * META_LENGTH_UNIT;
        block.resize(padded, 0);
        let mut out = vec![(padded / META_LENGTH_UNIT) as u8];
        out.extend(block);
        out
    }

    #[test]
    fn strips_metadata_and_publishes_title() {
        let mut data = b"abcd".to_vec();
        data.extend(meta_block("StreamTitle='x';"));
        data.extend_from_slice(b"efgh");
        let slot = TitleSlot::default();
        let mut reader = IcyReader::new(&data[..], 4, Arc::clone(&slot));
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"abcdefgh");
        assert_eq!(slot.lock().unwrap().as_deref(), Some("x"));
    }

    #[test]
    fn handles_empty_blocks_and_partial_reads() {
        let mut data = b"1234".to_vec();
        data.push(0); // empty metadata block
        data.extend_from_slice(b"5678");
        data.extend(meta_block("StreamTitle='Song - Artist';StreamUrl='';"));
        data.extend_from_slice(b"9");
        let slot = TitleSlot::default();
        let mut reader = IcyReader::new(&data[..], 4, Arc::clone(&slot));
        let mut out = Vec::new();
        let mut chunk = [0u8; 3];
        loop {
            let got = reader.read(&mut chunk).unwrap();
            if got == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..got]);
        }
        assert_eq!(out, b"123456789");
        assert_eq!(slot.lock().unwrap().as_deref(), Some("Song - Artist"));
    }

    #[test]
    fn metaint_zero_passes_bytes_through() {
        let data = b"raw audio bytes";
        let mut reader = IcyReader::new(&data[..], 0, TitleSlot::default());
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn parses_stream_title_variants() {
        assert_eq!(
            parse_stream_title("StreamTitle='A - B';StreamUrl='';"),
            Some("A - B".into())
        );
        assert_eq!(
            parse_stream_title("StreamTitle='  spaced  ';"),
            Some("spaced".into())
        );
        assert_eq!(
            parse_stream_title("StreamTitle='unterminated"),
            Some("unterminated".into())
        );
        assert_eq!(parse_stream_title("StreamTitle='';"), None);
        assert_eq!(parse_stream_title("StreamUrl='x';"), None);
    }

    #[test]
    fn maps_mime_types_to_extensions() {
        assert_eq!(extension_for_mime("audio/mpeg"), Some("mp3"));
        assert_eq!(extension_for_mime("audio/aacp; charset=utf-8"), Some("aac"));
        assert_eq!(extension_for_mime("application/ogg"), Some("ogg"));
        assert_eq!(extension_for_mime("text/html"), None);
        assert_eq!(extension_for_mime(""), None);
    }
}
