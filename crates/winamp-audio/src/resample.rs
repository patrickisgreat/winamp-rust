//! Simple stateful cubic (Catmull-Rom) resampler for interleaved stereo audio.

pub struct Resampler {
    ratio: f64, // input frames per output frame
    pos: f64,   // fractional read position relative to history start
    hist: Vec<f32>, // interleaved stereo history (last 3 frames + unconsumed input)
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Self { ratio: in_rate as f64 / out_rate as f64, pos: 1.0, hist: vec![0.0; 6] }
    }

    pub fn is_identity(&self) -> bool {
        (self.ratio - 1.0).abs() < 1e-9
    }

    pub fn reset(&mut self) {
        self.pos = 1.0;
        self.hist.clear();
        self.hist.resize(6, 0.0);
    }

    /// Resample `input` (interleaved stereo) appending to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.is_identity() {
            out.extend_from_slice(input);
            return;
        }
        self.hist.extend_from_slice(input);
        let frames = self.hist.len() / 2;
        // Need indices i-1, i, i+1, i+2 => i+2 <= frames-1
        while (self.pos.floor() as usize) + 2 < frames {
            let i = self.pos.floor() as usize;
            let t = (self.pos - i as f64) as f32;
            for ch in 0..2 {
                let p0 = self.hist[(i - 1) * 2 + ch];
                let p1 = self.hist[i * 2 + ch];
                let p2 = self.hist[(i + 1) * 2 + ch];
                let p3 = self.hist[(i + 2) * 2 + ch];
                out.push(catmull_rom(p0, p1, p2, p3, t));
            }
            self.pos += self.ratio;
        }
        // Drop consumed history, keeping one frame before the current position.
        let keep_from = (self.pos.floor() as usize).saturating_sub(1);
        if keep_from > 0 {
            self.hist.drain(..keep_from * 2);
            self.pos -= keep_from as f64;
        }
    }
}

#[inline]
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_expected_count() {
        let mut r = Resampler::new(44100, 48000);
        let input: Vec<f32> = (0..44100 * 2).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut out = Vec::new();
        for chunk in input.chunks(2048) {
            r.process(chunk, &mut out);
        }
        let frames = out.len() / 2;
        assert!((frames as i64 - 48000).abs() < 8, "got {frames}");
    }
}
