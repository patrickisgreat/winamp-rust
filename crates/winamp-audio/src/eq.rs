//! Winamp-style 10 band graphic equalizer built from peaking biquad filters.

/// Band center frequencies used by Winamp 2.x.
pub const BAND_FREQS: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];
pub const BAND_LABELS: [&str; 10] = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];
pub const NUM_BANDS: usize = 10;
pub const MAX_GAIN_DB: f32 = 12.0;

/// Equalizer settings as the user sees them (gains in dB, -12..=12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f32,
    pub bands_db: [f32; NUM_BANDS],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self { enabled: false, preamp_db: 0.0, bands_db: [0.0; NUM_BANDS] }
    }
}

impl EqSettings {
    pub fn flat() -> Self {
        Self::default()
    }
    pub fn clamp(&mut self) {
        self.preamp_db = self.preamp_db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
        for b in &mut self.bands_db {
            *b = b.clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
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
        let f = freq.min(nyquist * 0.95);
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * f / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha / a;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
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
    filters: [[Biquad; NUM_BANDS]; 2],
    preamp_gain: f32,
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

    pub fn apply(&mut self, mut s: EqSettings) {
        s.clamp();
        if s != self.settings {
            self.settings = s;
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        self.preamp_gain = 10f32.powf(self.settings.preamp_db / 20.0);
        for (i, &freq) in BAND_FREQS.iter().enumerate() {
            let gain = self.settings.bands_db[i];
            self.active[i] = gain.abs() > 0.01;
            // Narrower Q for the low bands keeps them from smearing into each other.
            let q = if i < 2 { 1.0 } else { 1.4 };
            for ch in 0..2 {
                let old = self.filters[ch][i];
                let mut f = Biquad::peaking(self.sample_rate, freq, q, gain);
                f.z1 = old.z1;
                f.z2 = old.z2;
                self.filters[ch][i] = f;
            }
        }
    }

    /// Process interleaved stereo samples in place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.settings.enabled {
            return;
        }
        for frame in samples.chunks_exact_mut(2) {
            for (ch, s) in frame.iter_mut().enumerate() {
                let mut x = *s * self.preamp_gain;
                for i in 0..NUM_BANDS {
                    if self.active[i] {
                        x = self.filters[ch][i].process(x);
                    }
                }
                *s = x;
            }
        }
    }
}

/// Parse a Winamp `.eqf` / `.q1` preset file. Returns (name, settings) pairs.
///
/// Format: "Winamp EQ library file v1.1" + 0x1A, then for each preset: 257 bytes
/// of NUL-padded name, 10 band bytes, 1 preamp byte. Bytes are 0..=63 where 0 = +20 dB
/// and 63 = -20 dB in Winamp's file format (we rescale to +-12 dB).
pub fn parse_eqf(data: &[u8]) -> Vec<(String, EqSettings)> {
    const HEADER: &[u8] = b"Winamp EQ library file v1.1\x1A";
    let mut out = Vec::new();
    if data.len() < HEADER.len() || &data[..HEADER.len()] != HEADER {
        return out;
    }
    let mut pos = HEADER.len();
    // Older files may have "!--" after the header; skip to the first preset block.
    if data.get(pos..pos + 3) == Some(b"!--") {
        pos += 3;
    }
    while pos + 257 + 11 <= data.len() {
        let name_bytes = &data[pos..pos + 257];
        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(257);
        let name = String::from_utf8_lossy(&name_bytes[..end]).to_string();
        pos += 257;
        let mut s = EqSettings { enabled: true, ..Default::default() };
        for i in 0..NUM_BANDS {
            s.bands_db[i] = eqf_byte_to_db(data[pos + i]);
        }
        s.preamp_db = eqf_byte_to_db(data[pos + 10]);
        pos += 11;
        out.push((name, s));
    }
    out
}

fn eqf_byte_to_db(b: u8) -> f32 {
    // 0 => +12 dB, 31/32 => 0 dB, 63 => -12 dB
    let v = b.min(63) as f32;
    (1.0 - v / 63.0) * 2.0 * MAX_GAIN_DB - MAX_GAIN_DB
}

fn db_to_eqf_byte(db: f32) -> u8 {
    let t = (db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB) + MAX_GAIN_DB) / (2.0 * MAX_GAIN_DB);
    ((1.0 - t) * 63.0).round() as u8
}

/// Serialize presets as a Winamp `.eqf` file.
pub fn write_eqf(presets: &[(String, EqSettings)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"Winamp EQ library file v1.1\x1A!--");
    for (name, s) in presets {
        let mut name_buf = [0u8; 257];
        let bytes = name.as_bytes();
        let n = bytes.len().min(256);
        name_buf[..n].copy_from_slice(&bytes[..n]);
        out.extend_from_slice(&name_buf);
        for i in 0..NUM_BANDS {
            out.push(db_to_eqf_byte(s.bands_db[i]));
        }
        out.push(db_to_eqf_byte(s.preamp_db));
    }
    out
}

/// Built-in presets, mirroring the names Winamp shipped with.
pub fn builtin_presets() -> Vec<(String, EqSettings)> {
    fn p(name: &str, bands: [f32; 10]) -> (String, EqSettings) {
        (name.to_string(), EqSettings { enabled: true, preamp_db: 0.0, bands_db: bands })
    }
    vec![
        p("Classical", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -4.3, -4.3, -4.3, -5.8]),
        p("Club", [0.0, 0.0, 4.8, 3.4, 3.4, 3.4, 1.9, 0.0, 0.0, 0.0]),
        p("Dance", [5.8, 4.3, 1.4, 0.0, 0.0, -3.4, -4.3, -4.3, 0.0, 0.0]),
        p("Flat", [0.0; 10]),
        p("Full Bass", [-4.8, 5.8, 5.8, 3.4, 1.0, -2.4, -4.8, -6.3, -6.7, -6.7]),
        p("Full Bass & Treble", [4.3, 3.4, 0.0, -4.3, -2.9, 1.0, 4.8, 6.7, 7.2, 7.2]),
        p("Full Treble", [-5.8, -5.8, -5.8, -2.4, 1.4, 6.7, 9.6, 9.6, 9.6, 10.1]),
        p("Laptop Speakers / Headphones", [2.9, 6.7, 3.4, -2.0, -1.4, 1.0, 2.9, 5.8, 7.7, 8.6]),
        p("Large Hall", [6.2, 6.2, 3.4, 3.4, 0.0, -2.9, -2.9, -2.9, 0.0, 0.0]),
        p("Live", [-2.9, 0.0, 2.4, 3.4, 3.4, 3.4, 2.4, 1.4, 1.4, 1.4]),
        p("Party", [4.3, 4.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.3, 4.3]),
        p("Pop", [-1.0, 2.9, 4.3, 4.8, 3.4, 0.0, -1.4, -1.4, -1.0, -1.0]),
        p("Reggae", [0.0, 0.0, 0.0, -3.4, 0.0, 3.8, 3.8, 0.0, 0.0, 0.0]),
        p("Rock", [4.8, 2.9, -3.4, -4.8, -2.0, 2.4, 5.3, 6.7, 6.7, 6.7]),
        p("Ska", [-1.4, -2.9, -2.4, 0.0, 2.4, 3.4, 5.3, 5.8, 6.7, 5.8]),
        p("Soft", [2.9, 1.0, 0.0, -1.4, 0.0, 2.4, 4.8, 5.8, 6.7, 7.2]),
        p("Soft Rock", [2.4, 2.4, 1.4, 0.0, -2.4, -3.4, -2.0, 0.0, 1.4, 5.3]),
        p("Techno", [4.8, 3.4, 0.0, -3.4, -2.9, 0.0, 4.8, 5.8, 5.8, 5.3]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eqf_roundtrip() {
        let presets = builtin_presets();
        let bytes = write_eqf(&presets);
        let parsed = parse_eqf(&bytes);
        assert_eq!(parsed.len(), presets.len());
        for ((n1, s1), (n2, s2)) in presets.iter().zip(parsed.iter()) {
            assert_eq!(n1, n2);
            for i in 0..NUM_BANDS {
                assert!((s1.bands_db[i] - s2.bands_db[i]).abs() < 0.4);
            }
        }
    }

    #[test]
    fn flat_eq_is_identity() {
        let mut eq = Equalizer::new(44100);
        eq.apply(EqSettings { enabled: true, ..Default::default() });
        let mut buf: Vec<f32> = (0..200).map(|i| ((i as f32) * 0.1).sin()).collect();
        let orig = buf.clone();
        eq.process(&mut buf);
        for (a, b) in buf.iter().zip(orig.iter()) {
            assert!((a - b).abs() < 1e-4);
        }
    }
}
