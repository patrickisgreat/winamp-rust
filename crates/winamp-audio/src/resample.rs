//! Simple stateful cubic (Catmull-Rom) resampler for interleaved stereo audio.

const CHANNELS: usize = 2;
/// Frames of history kept before the read position so the cubic kernel
/// always has a neighbour on each side.
const WARMUP_FRAMES: usize = 3;

pub struct Resampler {
    /// Input frames consumed per output frame.
    ratio: f64,
    /// Fractional read position, in frames, relative to the start of `history`.
    read_pos: f64,
    /// Interleaved stereo history: the last few frames plus any unconsumed input.
    history: Vec<f32>,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            ratio: f64::from(input_rate) / f64::from(output_rate),
            read_pos: 1.0,
            history: vec![0.0; WARMUP_FRAMES * CHANNELS],
        }
    }

    /// True when input and output rates match and samples pass through untouched.
    pub fn is_identity(&self) -> bool {
        (self.ratio - 1.0).abs() < 1e-9
    }

    /// Forget all history, e.g. after a seek.
    pub fn reset(&mut self) {
        self.read_pos = 1.0;
        self.history.clear();
        self.history.resize(WARMUP_FRAMES * CHANNELS, 0.0);
    }

    /// Resample `input` (interleaved stereo), appending the result to `output`.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        if self.is_identity() {
            output.extend_from_slice(input);
            return;
        }
        self.history.extend_from_slice(input);
        let frames = self.history.len() / CHANNELS;
        // The kernel reads frames i-1 ..= i+2, so we need i+2 <= frames-1.
        while (self.read_pos.floor() as usize) + 2 < frames {
            let i = self.read_pos.floor() as usize;
            let t = (self.read_pos - i as f64) as f32;
            for ch in 0..CHANNELS {
                let p0 = self.history[(i - 1) * CHANNELS + ch];
                let p1 = self.history[i * CHANNELS + ch];
                let p2 = self.history[(i + 1) * CHANNELS + ch];
                let p3 = self.history[(i + 2) * CHANNELS + ch];
                output.push(catmull_rom(p0, p1, p2, p3, t));
            }
            self.read_pos += self.ratio;
        }
        // Drop consumed history, keeping one frame before the current position.
        let keep_from = (self.read_pos.floor() as usize).saturating_sub(1);
        if keep_from > 0 {
            self.history.drain(..keep_from * CHANNELS);
            self.read_pos -= keep_from as f64;
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

    fn stereo_sine(frames: usize) -> Vec<f32> {
        (0..frames * CHANNELS)
            .map(|i| (i as f32 * 0.01).sin())
            .collect()
    }

    #[test]
    fn upsampling_produces_expected_frame_count() {
        let mut resampler = Resampler::new(44100, 48000);
        let mut out = Vec::new();
        for chunk in stereo_sine(44100).chunks(2048) {
            resampler.process(chunk, &mut out);
        }
        let frames = out.len() / CHANNELS;
        assert!((frames as i64 - 48000).abs() < 8, "got {frames}");
    }

    #[test]
    fn downsampling_produces_expected_frame_count() {
        let mut resampler = Resampler::new(48000, 44100);
        let mut out = Vec::new();
        for chunk in stereo_sine(48000).chunks(1000) {
            resampler.process(chunk, &mut out);
        }
        let frames = out.len() / CHANNELS;
        assert!((frames as i64 - 44100).abs() < 8, "got {frames}");
    }

    #[test]
    fn identity_passes_samples_through_unchanged() {
        let mut resampler = Resampler::new(44100, 44100);
        assert!(resampler.is_identity());
        let input = stereo_sine(100);
        let mut out = Vec::new();
        resampler.process(&input, &mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn constant_signal_stays_constant() {
        let mut resampler = Resampler::new(44100, 48000);
        let input = vec![0.25f32; 2000 * CHANNELS];
        let mut out = Vec::new();
        resampler.process(&input, &mut out);
        // Skip the zero-filled warm-up frames at the very start.
        for sample in out.iter().skip(8 * CHANNELS) {
            assert!((sample - 0.25).abs() < 1e-5, "sample {sample}");
        }
    }

    #[test]
    fn reset_discards_history() {
        let mut resampler = Resampler::new(44100, 48000);
        let mut out = Vec::new();
        resampler.process(&vec![1.0; 500 * CHANNELS], &mut out);
        resampler.reset();
        assert_eq!(resampler.history.len(), WARMUP_FRAMES * CHANNELS);
        assert!(resampler.history.iter().all(|&s| s == 0.0));
    }
}
