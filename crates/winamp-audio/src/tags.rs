//! Metadata reading via lofty.

use std::path::Path;
use std::time::Duration;

use lofty::prelude::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub track_number: Option<u32>,
    pub duration: Option<Duration>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub file_size: Option<u64>,
}

impl TrackMeta {
    /// "Artist - Title" when tags are present, otherwise the file stem.
    pub fn display_title(&self, path: &Path) -> String {
        match (&self.artist, &self.title) {
            (Some(a), Some(t)) if !a.is_empty() && !t.is_empty() => format!("{a} - {t}"),
            (_, Some(t)) if !t.is_empty() => t.clone(),
            _ => path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
        }
    }
}

pub fn read_meta(path: &Path) -> TrackMeta {
    let mut meta = TrackMeta::default();
    meta.file_size = std::fs::metadata(path).ok().map(|m| m.len());
    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(e) => {
            log::debug!("lofty failed on {}: {e}", path.display());
            return meta;
        }
    };
    let props = tagged.properties();
    meta.duration = Some(props.duration()).filter(|d| !d.is_zero());
    meta.bitrate_kbps = props.audio_bitrate();
    meta.sample_rate = props.sample_rate();
    meta.channels = props.channels();
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        meta.title = tag.title().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        meta.artist = tag.artist().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        meta.album = tag.album().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        meta.year = tag.year();
        meta.genre = tag.genre().map(|s| s.to_string());
        meta.comment = tag.comment().map(|s| s.to_string());
        meta.track_number = tag.track();
    }
    meta
}
