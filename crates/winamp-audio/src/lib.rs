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
pub fn format_duration(d: std::time::Duration) -> String {
    let total = d.as_secs();
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
