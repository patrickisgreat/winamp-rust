//! Audio engine for the Winamp rewrite: decoding, streaming, EQ, output and
//! visualisation analysis.

pub mod decode;
pub mod engine;
pub mod eq;
pub mod resample;
pub mod ring;
pub mod stream;
pub mod tags;
pub mod vis;

pub use decode::{is_supported_path, Source, TrackInfo, SUPPORTED_EXTENSIONS};
pub use engine::{Event, PlayState, Player};
pub use eq::{EqSettings, BAND_FREQS, BAND_LABELS, NUM_BANDS};
pub use tags::{read_meta, TrackMeta};

/// Format a duration as Winamp does: `m:ss`, or `h:mm:ss` beyond an hour.
pub fn format_duration(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let (hours, minutes, seconds) = (total_secs / 3600, (total_secs / 60) % 60, total_secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;
    use std::time::Duration;

    #[test]
    fn formats_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::ZERO), "0:00");
        assert_eq!(format_duration(Duration::from_secs(5)), "0:05");
        assert_eq!(format_duration(Duration::from_secs(65)), "1:05");
        assert_eq!(format_duration(Duration::from_secs(59 * 60 + 59)), "59:59");
    }

    #[test]
    fn formats_hours_once_past_sixty_minutes() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "1:00:00");
        assert_eq!(
            format_duration(Duration::from_secs(3600 + 7 * 60 + 3)),
            "1:07:03"
        );
    }

    #[test]
    fn truncates_fractional_seconds() {
        assert_eq!(format_duration(Duration::from_millis(1999)), "0:01");
    }
}
