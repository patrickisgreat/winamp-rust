//! Winamp-style 10 band graphic equalizer built from peaking biquad filters.

/// Band center frequencies used by Winamp 2.x.
pub const BAND_FREQS: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];
pub const BAND_LABELS: [&str; 10] = [
    "60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K",
];
pub const NUM_BANDS: usize = 10;
pub const MAX_GAIN_DB: f32 = 12.0;

/// Gains below this magnitude are treated as flat and their filter skipped.
const FLAT_THRESHOLD_DB: f32 = 0.01;

/// Equalizer settings as the user sees them (gains in dB, -12..=12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f32,
    pub bands_db: [f32; NUM_BANDS],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            preamp_db: 0.0,
            bands_db: [0.0; NUM_BANDS],
        }
    }
}

impl EqSettings {
    pub fn flat() -> Self {
        Self::default()
    }

    /// The same settings with every gain limited to the +-12 dB range.
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            preamp_db: clamp_gain(self.preamp_db),
            bands_db: self.bands_db.map(clamp_gain),
        }
    }
}

fn clamp_gain(db: f32) -> f32 {
    db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB)
}

/// A named set of equalizer gains, as stored in `.eqf` files and menus.
#[derive(Clone, Debug, PartialEq)]
pub struct EqPreset {
    pub name: String,
    pub settings: EqSettings,
}

impl EqPreset {
    pub fn new(name: impl Into<String>, settings: EqSettings) -> Self {
        Self {
            name: name.into(),
            settings,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn peaking(sample_rate: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        let nyquist = sample_rate * 0.5;
        let freq = freq.min(nyquist * 0.95);
        let amplitude = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let b0 = 1.0 + alpha * amplitude;
        let b1 = -2.0 * cos;
        let b2 = 1.0 - alpha * amplitude;
        let a0 = 1.0 + alpha / amplitude;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha / amplitude;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Same coefficients, but carrying over the filter state of `previous`
    /// so changing a gain mid-stream does not click.
    fn continuing(self, previous: Biquad) -> Self {
        Self {
            z1: previous.z1,
            z2: previous.z2,
            ..self
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        // Transposed direct form II
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Stateful equalizer processing interleaved stereo audio.
pub struct Equalizer {
    sample_rate: f32,
    settings: EqSettings,
    /// One filter per band, per channel (left, right).
    filters: [[Biquad; NUM_BANDS]; 2],
    preamp_gain: f32,
    /// Bands whose gain is non-zero; flat bands are skipped for speed.
    active: [bool; NUM_BANDS],
}

impl Equalizer {
    pub fn new(sample_rate: u32) -> Self {
        let mut eq = Self {
            sample_rate: sample_rate as f32,
            settings: EqSettings::default(),
            filters: [[Biquad::default(); NUM_BANDS]; 2],
            preamp_gain: 1.0,
            active: [false; NUM_BANDS],
        };
        eq.rebuild();
        eq
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        if (self.sample_rate - sample_rate as f32).abs() > 0.5 {
            self.sample_rate = sample_rate as f32;
            self.rebuild();
        }
    }

    pub fn settings(&self) -> &EqSettings {
        &self.settings
    }

    pub fn apply(&mut self, settings: EqSettings) {
        let settings = settings.clamped();
        if settings != self.settings {
            self.settings = settings;
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        self.preamp_gain = 10f32.powf(self.settings.preamp_db / 20.0);
        for (band, &freq) in BAND_FREQS.iter().enumerate() {
            let gain = self.settings.bands_db[band];
            self.active[band] = gain.abs() > FLAT_THRESHOLD_DB;
            // Narrower Q for the low bands keeps them from smearing into each other.
            let q = if band < 2 { 1.0 } else { 1.4 };
            for channel_filters in &mut self.filters {
                let previous = channel_filters[band];
                channel_filters[band] =
                    Biquad::peaking(self.sample_rate, freq, q, gain).continuing(previous);
            }
        }
    }

    /// Process interleaved stereo samples in place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.settings.enabled {
            return;
        }
        for frame in samples.chunks_exact_mut(2) {
            for (channel, sample) in frame.iter_mut().enumerate() {
                let mut x = *sample * self.preamp_gain;
                for (filter, &active) in self.filters[channel].iter_mut().zip(&self.active) {
                    if active {
                        x = filter.process(x);
                    }
                }
                *sample = x;
            }
        }
    }
}

// --- .eqf preset files -------------------------------------------------------

const EQF_HEADER: &[u8] = b"Winamp EQ library file v1.1\x1A";
/// Some writers put this marker between the header and the first preset.
const EQF_SEPARATOR: &[u8] = b"!--";
const EQF_NAME_LEN: usize = 257;
/// Ten band bytes followed by one preamp byte.
const EQF_GAINS_LEN: usize = NUM_BANDS + 1;
const EQF_MAX_BYTE: u8 = 63;

/// Parse a Winamp `.eqf` / `.q1` preset file.
///
/// Format: the header, then for each preset 257 bytes of NUL-padded name,
/// 10 band bytes and 1 preamp byte. Bytes run 0..=63 where 0 is the maximum
/// boost and 63 the maximum cut (Winamp used +-20 dB; we rescale to +-12 dB).
/// Anything that is not an `.eqf` file yields an empty list.
pub fn parse_eqf(data: &[u8]) -> Vec<EqPreset> {
    let Some(mut body) = data.strip_prefix(EQF_HEADER) else {
        return Vec::new();
    };
    body = body.strip_prefix(EQF_SEPARATOR).unwrap_or(body);
    body.chunks_exact(EQF_NAME_LEN + EQF_GAINS_LEN)
        .map(|record| {
            let (name_bytes, gains) = record.split_at(EQF_NAME_LEN);
            let name_end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(EQF_NAME_LEN);
            let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
            let mut bands_db = [0.0; NUM_BANDS];
            for (band, &byte) in bands_db.iter_mut().zip(&gains[..NUM_BANDS]) {
                *band = eqf_byte_to_db(byte);
            }
            let settings = EqSettings {
                enabled: true,
                preamp_db: eqf_byte_to_db(gains[NUM_BANDS]),
                bands_db,
            };
            EqPreset { name, settings }
        })
        .collect()
}

fn eqf_byte_to_db(byte: u8) -> f32 {
    // 0 => +12 dB, 31/32 => 0 dB, 63 => -12 dB
    let v = f32::from(byte.min(EQF_MAX_BYTE));
    (1.0 - v / f32::from(EQF_MAX_BYTE)) * 2.0 * MAX_GAIN_DB - MAX_GAIN_DB
}

fn db_to_eqf_byte(db: f32) -> u8 {
    let t = (clamp_gain(db) + MAX_GAIN_DB) / (2.0 * MAX_GAIN_DB);
    ((1.0 - t) * f32::from(EQF_MAX_BYTE)).round() as u8
}

/// Serialize presets as a Winamp `.eqf` file.
pub fn write_eqf(presets: &[EqPreset]) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        EQF_HEADER.len() + EQF_SEPARATOR.len() + presets.len() * (EQF_NAME_LEN + EQF_GAINS_LEN),
    );
    out.extend_from_slice(EQF_HEADER);
    out.extend_from_slice(EQF_SEPARATOR);
    for preset in presets {
        let mut name_field = [0u8; EQF_NAME_LEN];
        let name = preset.name.as_bytes();
        let copied = name.len().min(EQF_NAME_LEN - 1);
        name_field[..copied].copy_from_slice(&name[..copied]);
        out.extend_from_slice(&name_field);
        out.extend(
            preset
                .settings
                .bands_db
                .iter()
                .map(|&db| db_to_eqf_byte(db)),
        );
        out.push(db_to_eqf_byte(preset.settings.preamp_db));
    }
    out
}

/// Built-in presets, mirroring the names Winamp shipped with.
pub fn builtin_presets() -> Vec<EqPreset> {
    fn preset(name: &str, bands_db: [f32; NUM_BANDS]) -> EqPreset {
        EqPreset::new(
            name,
            EqSettings {
                enabled: true,
                preamp_db: 0.0,
                bands_db,
            },
        )
    }
    vec![
        preset(
            "Classical",
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -4.3, -4.3, -4.3, -5.8],
        ),
        preset("Club", [0.0, 0.0, 4.8, 3.4, 3.4, 3.4, 1.9, 0.0, 0.0, 0.0]),
        preset(
            "Dance",
            [5.8, 4.3, 1.4, 0.0, 0.0, -3.4, -4.3, -4.3, 0.0, 0.0],
        ),
        preset("Flat", [0.0; 10]),
        preset(
            "Full Bass",
            [-4.8, 5.8, 5.8, 3.4, 1.0, -2.4, -4.8, -6.3, -6.7, -6.7],
        ),
        preset(
            "Full Bass & Treble",
            [4.3, 3.4, 0.0, -4.3, -2.9, 1.0, 4.8, 6.7, 7.2, 7.2],
        ),
        preset(
            "Full Treble",
            [-5.8, -5.8, -5.8, -2.4, 1.4, 6.7, 9.6, 9.6, 9.6, 10.1],
        ),
        preset(
            "Laptop Speakers / Headphones",
            [2.9, 6.7, 3.4, -2.0, -1.4, 1.0, 2.9, 5.8, 7.7, 8.6],
        ),
        preset(
            "Large Hall",
            [6.2, 6.2, 3.4, 3.4, 0.0, -2.9, -2.9, -2.9, 0.0, 0.0],
        ),
        preset("Live", [-2.9, 0.0, 2.4, 3.4, 3.4, 3.4, 2.4, 1.4, 1.4, 1.4]),
        preset("Party", [4.3, 4.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.3, 4.3]),
        preset(
            "Pop",
            [-1.0, 2.9, 4.3, 4.8, 3.4, 0.0, -1.4, -1.4, -1.0, -1.0],
        ),
        preset(
            "Reggae",
            [0.0, 0.0, 0.0, -3.4, 0.0, 3.8, 3.8, 0.0, 0.0, 0.0],
        ),
        preset(
            "Rock",
            [4.8, 2.9, -3.4, -4.8, -2.0, 2.4, 5.3, 6.7, 6.7, 6.7],
        ),
        preset("Ska", [-1.4, -2.9, -2.4, 0.0, 2.4, 3.4, 5.3, 5.8, 6.7, 5.8]),
        preset("Soft", [2.9, 1.0, 0.0, -1.4, 0.0, 2.4, 4.8, 5.8, 6.7, 7.2]),
        preset(
            "Soft Rock",
            [2.4, 2.4, 1.4, 0.0, -2.4, -3.4, -2.0, 0.0, 1.4, 5.3],
        ),
        preset(
            "Techno",
            [4.8, 3.4, 0.0, -3.4, -2.9, 0.0, 4.8, 5.8, 5.8, 5.3],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, sample_rate: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = (2.0 * std::f32::consts::PI * hz * i as f32 / sample_rate).sin();
                [s, s]
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn eqf_roundtrip_keeps_names_and_gains() {
        let presets = builtin_presets();
        let bytes = write_eqf(&presets);
        let parsed = parse_eqf(&bytes);
        assert_eq!(parsed.len(), presets.len());
        for (original, back) in presets.iter().zip(&parsed) {
            assert_eq!(original.name, back.name);
            assert!(back.settings.enabled);
            for band in 0..NUM_BANDS {
                assert!(
                    (original.settings.bands_db[band] - back.settings.bands_db[band]).abs() < 0.4
                );
            }
        }
    }

    #[test]
    fn eqf_rejects_other_files_and_truncated_records() {
        assert!(parse_eqf(b"").is_empty());
        assert!(parse_eqf(b"not an eq file").is_empty());
        let mut truncated = write_eqf(&builtin_presets()[..1]);
        truncated.truncate(truncated.len() - 1);
        assert!(parse_eqf(&truncated).is_empty());
    }

    #[test]
    fn eqf_byte_scale_endpoints() {
        assert_eq!(eqf_byte_to_db(0), MAX_GAIN_DB);
        assert_eq!(eqf_byte_to_db(63), -MAX_GAIN_DB);
        assert_eq!(eqf_byte_to_db(200), -MAX_GAIN_DB);
        assert_eq!(db_to_eqf_byte(MAX_GAIN_DB), 0);
        assert_eq!(db_to_eqf_byte(-MAX_GAIN_DB), 63);
        assert_eq!(db_to_eqf_byte(100.0), 0);
    }

    #[test]
    fn eqf_truncates_overlong_names_without_panicking() {
        let long_name = "x".repeat(1000);
        let bytes = write_eqf(&[EqPreset::new(long_name, EqSettings::flat())]);
        let parsed = parse_eqf(&bytes);
        assert_eq!(parsed[0].name.len(), EQF_NAME_LEN - 1);
    }

    #[test]
    fn there_are_eighteen_builtin_presets_with_unique_names() {
        let presets = builtin_presets();
        assert_eq!(presets.len(), 18);
        let mut names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 18);
    }

    #[test]
    fn clamped_limits_every_gain() {
        let wild = EqSettings {
            enabled: true,
            preamp_db: 40.0,
            bands_db: [-99.0; NUM_BANDS],
        };
        let clamped = wild.clamped();
        assert_eq!(clamped.preamp_db, MAX_GAIN_DB);
        assert!(clamped.bands_db.iter().all(|&b| b == -MAX_GAIN_DB));
        assert!(clamped.enabled);
    }

    #[test]
    fn flat_eq_is_identity() {
        let mut eq = Equalizer::new(44100);
        eq.apply(EqSettings {
            enabled: true,
            ..Default::default()
        });
        let mut buf: Vec<f32> = (0..200).map(|i| ((i as f32) * 0.1).sin()).collect();
        let orig = buf.clone();
        eq.process(&mut buf);
        for (a, b) in buf.iter().zip(orig.iter()) {
            assert!((a - b).abs() < 1e-4);
        }
    }

    #[test]
    fn disabled_eq_leaves_audio_untouched_even_with_gain() {
        let mut eq = Equalizer::new(44100);
        eq.apply(EqSettings {
            enabled: false,
            preamp_db: 12.0,
            bands_db: [12.0; NUM_BANDS],
        });
        let mut buf = sine(1000.0, 44100.0, 500);
        let orig = buf.clone();
        eq.process(&mut buf);
        assert_eq!(buf, orig);
    }

    #[test]
    fn boosting_a_band_raises_a_tone_in_it_and_not_elsewhere() {
        let mut settings = EqSettings {
            enabled: true,
            ..Default::default()
        };
        settings.bands_db[4] = 12.0; // 1 kHz
        let mut eq = Equalizer::new(44100);
        eq.apply(settings);
        let mut in_band = sine(1000.0, 44100.0, 8000);
        let before = rms(&in_band);
        eq.process(&mut in_band);
        assert!(
            rms(&in_band[4000..]) > before * 2.5,
            "1 kHz should be boosted by about 12 dB"
        );

        let mut eq = Equalizer::new(44100);
        eq.apply(settings);
        let mut out_of_band = sine(60.0, 44100.0, 8000);
        let before = rms(&out_of_band);
        eq.process(&mut out_of_band);
        assert!(
            (rms(&out_of_band[4000..]) - before).abs() < before * 0.15,
            "60 Hz should be left alone"
        );
    }

    #[test]
    fn preamp_scales_output() {
        let mut eq = Equalizer::new(44100);
        eq.apply(EqSettings {
            enabled: true,
            preamp_db: -6.0,
            ..Default::default()
        });
        let mut buf = vec![0.5f32; 100];
        eq.process(&mut buf);
        let expected = 0.5 * 10f32.powf(-6.0 / 20.0);
        assert!(buf.iter().all(|s| (s - expected).abs() < 1e-5));
    }
}
