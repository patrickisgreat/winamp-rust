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
    /// "Artist - Title" when both tags are present, the title alone when only
    /// it is, otherwise the file stem (or the whole path if there is none).
    pub fn display_title(&self, path: &Path) -> String {
        match (&self.artist, &self.title) {
            (Some(artist), Some(title)) if !artist.is_empty() && !title.is_empty() => {
                format!("{artist} - {title}")
            }
            (_, Some(title)) if !title.is_empty() => title.clone(),
            _ => path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned()),
        }
    }
}

/// Read tags and audio properties from a local file. Never fails: anything
/// lofty cannot read simply comes back with the fields left empty.
pub fn read_meta(path: impl AsRef<Path>) -> TrackMeta {
    let path = path.as_ref();
    let mut meta = TrackMeta {
        file_size: std::fs::metadata(path).ok().map(|m| m.len()),
        ..TrackMeta::default()
    };
    let tagged = match lofty::read_from_path(path) {
        Ok(tagged) => tagged,
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
        meta.title = non_empty(tag.title());
        meta.artist = non_empty(tag.artist());
        meta.album = non_empty(tag.album());
        meta.year = tag.year();
        meta.genre = tag.genre().map(|s| s.to_string());
        meta.comment = tag.comment().map(|s| s.to_string());
        meta.track_number = tag.track();
    }
    meta
}

/// Trim a tag value and drop it entirely when nothing is left.
fn non_empty(value: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(artist: Option<&str>, title: Option<&str>) -> TrackMeta {
        TrackMeta {
            artist: artist.map(str::to_string),
            title: title.map(str::to_string),
            ..TrackMeta::default()
        }
    }

    #[test]
    fn display_title_prefers_artist_and_title() {
        let path = Path::new("/music/01 - song.mp3");
        assert_eq!(
            meta(Some("Daft Punk"), Some("Around the World")).display_title(path),
            "Daft Punk - Around the World"
        );
    }

    #[test]
    fn display_title_falls_back_to_title_then_file_stem() {
        let path = Path::new("/music/01 - song.mp3");
        assert_eq!(
            meta(None, Some("Only Title")).display_title(path),
            "Only Title"
        );
        assert_eq!(
            meta(Some(""), Some("Only Title")).display_title(path),
            "Only Title"
        );
        assert_eq!(meta(None, None).display_title(path), "01 - song");
        assert_eq!(
            meta(Some("Artist"), Some("")).display_title(path),
            "01 - song"
        );
    }

    #[test]
    fn read_meta_on_missing_file_is_empty() {
        let meta = read_meta("/definitely/not/here.mp3");
        assert_eq!(meta, TrackMeta::default());
    }
}
