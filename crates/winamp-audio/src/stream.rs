//! HTTP streaming (Shoutcast / Icecast) with ICY metadata extraction.

use std::io::{self, Read};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};

/// Information learned from the HTTP response headers of a stream.
#[derive(Clone, Debug, Default)]
pub struct StreamInfo {
    pub name: Option<String>,
    pub genre: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub content_type: Option<String>,
    pub metaint: Option<usize>,
}

/// Shared slot that receives "StreamTitle" updates from the ICY metadata.
pub type TitleSlot = Arc<Mutex<Option<String>>>;

/// A reader that strips ICY metadata blocks out of a Shoutcast stream.
pub struct IcyReader<R: Read> {
    inner: R,
    metaint: usize,
    until_meta: usize,
    title: TitleSlot,
}

impl<R: Read> IcyReader<R> {
    pub fn new(inner: R, metaint: usize, title: TitleSlot) -> Self {
        Self { inner, metaint, until_meta: metaint, title }
    }

    /// Returns Ok(false) at end of stream.
    fn read_meta_block(&mut self) -> io::Result<bool> {
        let mut len = [0u8; 1];
        if self.inner.read(&mut len)? == 0 {
            return Ok(false);
        }
        let n = len[0] as usize * 16;
        if n == 0 {
            return Ok(true);
        }
        let mut buf = vec![0u8; n];
        self.inner.read_exact(&mut buf)?;
        let text = String::from_utf8_lossy(&buf);
        if let Some(title) = parse_stream_title(&text) {
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
        let n = self.inner.read(&mut buf[..want])?;
        self.until_meta -= n;
        Ok(n)
    }
}

pub fn parse_stream_title(meta: &str) -> Option<String> {
    let start = meta.find("StreamTitle='")? + "StreamTitle='".len();
    let rest = &meta[start..];
    let end = rest.find("';").unwrap_or(rest.len());
    let t = rest[..end].trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Open an HTTP(S) audio stream. Returns the reader (with ICY metadata stripped),
/// header-derived info and the shared title slot.
pub fn open(url: &str) -> Result<(Box<dyn Read + Send + Sync>, StreamInfo, TitleSlot)> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(20))
        .user_agent("WinampRust/0.1 (like Winamp/2.95)")
        .build();
    let resp = agent
        .get(url)
        .set("Icy-MetaData", "1")
        .call()
        .with_context(|| format!("opening {url}"))?;
    if resp.status() >= 400 {
        return Err(anyhow!("HTTP {} for {url}", resp.status()));
    }
    let mut info = StreamInfo::default();
    info.name = resp.header("icy-name").map(|s| s.to_string());
    info.genre = resp.header("icy-genre").map(|s| s.to_string());
    info.bitrate_kbps = resp.header("icy-br").and_then(|s| s.trim().parse().ok());
    info.content_type = resp.header("content-type").map(|s| s.to_lowercase());
    info.metaint = resp.header("icy-metaint").and_then(|s| s.trim().parse().ok());
    let title: TitleSlot = Arc::new(Mutex::new(None));
    let reader = resp.into_reader();
    let reader: Box<dyn Read + Send + Sync> = match info.metaint {
        Some(m) if m > 0 => Box::new(IcyReader::new(reader, m, title.clone())),
        _ => Box::new(reader),
    };
    Ok((reader, info, title))
}

/// Guess a container/codec extension hint from a MIME type.
pub fn extension_for_mime(mime: &str) -> Option<&'static str> {
    let m = mime.split(';').next().unwrap_or("").trim();
    Some(match m {
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
    let resp = ureq::get(url).call().with_context(|| format!("fetching {url}"))?;
    let mut reader = resp.into_reader().take(max_bytes as u64);
    let mut s = String::new();
    reader.read_to_string(&mut s)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_metadata() {
        // metaint = 4, one metadata block of 16 bytes
        let mut data = Vec::new();
        data.extend_from_slice(b"abcd");
        data.push(1);
        let mut meta = b"StreamTitle='x';".to_vec();
        meta.resize(16, 0);
        data.extend_from_slice(&meta);
        data.extend_from_slice(b"efgh");
        let slot = TitleSlot::default();
        let mut r = IcyReader::new(&data[..], 4, slot.clone());
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"abcdefgh");
        assert_eq!(slot.lock().unwrap().as_deref(), Some("x"));
    }
}
