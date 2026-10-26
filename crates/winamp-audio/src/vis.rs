//! Visualization analysis: spectrum analyser and oscilloscope sampling.

use std::f32::consts::PI;

/// Number of samples Winamp hands to visualisation plugins per frame.
pub const VIS_SAMPLES: usize = 576;
const FFT_SIZE: usize = 512;
/// Lowest frequency shown by the analyser.
const MIN_HZ: f32 = 40.0;
/// Highest frequency shown by the analyser (or Nyquist, if lower).
const MAX_HZ: f32 = 20_000.0;
/// Dynamic range mapped onto the 0..=1 bar height.
const RANGE_DB: f32 = 60.0;

/// Simple in-place radix-2 FFT (real input packed as complex).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    // Butterflies.
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        let (wr, wi) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (ur, ui) = (re[a], im[a]);
                let (xr, xi) = (re[b], im[b]);
                let vr = xr * cr - xi * ci;
                let vi = xr * ci + xi * cr;
                re[a] = ur + vr;
                im[a] = ui + vi;
                re[b] = ur - vr;
                im[b] = ui - vi;
                let next_cr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = next_cr;
            }
        }
        len <<= 1;
    }
}

/// Compute `bars` log-spaced spectrum magnitudes (0.0..=1.0) from mono samples.
pub fn spectrum(samples: &[f32], sample_rate: u32, bars: usize) -> Vec<f32> {
    let mut re = vec![0.0f32; FFT_SIZE];
    let mut im = vec![0.0f32; FFT_SIZE];
    let used = samples.len().min(FFT_SIZE);
    let newest = &samples[samples.len() - used..];
    for (i, (slot, &sample)) in re.iter_mut().zip(newest).enumerate() {
        let hann = 0.5 - 0.5 * (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos();
        *slot = sample * hann;
    }
    fft(&mut re, &mut im);
    let bins = FFT_SIZE / 2;
    let magnitudes: Vec<f32> = re[..bins]
        .iter()
        .zip(&im[..bins])
        .map(|(r, i)| (r * r + i * i).sqrt() / (FFT_SIZE as f32 / 4.0))
        .collect();
    let sample_rate = sample_rate.max(8000) as f32;
    let hz_per_bin = sample_rate / FFT_SIZE as f32;
    let max_hz = (sample_rate / 2.0).min(MAX_HZ);
    (0..bars)
        .map(|bar| {
            let low_hz = MIN_HZ * (max_hz / MIN_HZ).powf(bar as f32 / bars as f32);
            let high_hz = MIN_HZ * (max_hz / MIN_HZ).powf((bar + 1) as f32 / bars as f32);
            let first_bin = ((low_hz / hz_per_bin) as usize).clamp(1, bins - 1);
            let end_bin = ((high_hz / hz_per_bin) as usize).clamp(first_bin + 1, bins);
            let peak = magnitudes[first_bin..end_bin]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            // A dB-ish scale that looks like Winamp's analyser.
            let db = 20.0 * (peak + 1e-6).log10();
            ((db + RANGE_DB) / RANGE_DB).clamp(0.0, 1.0)
        })
        .collect()
}

/// Downsample mono audio to `width` oscilloscope points in -1.0..=1.0.
pub fn oscilloscope(samples: &[f32], width: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; width];
    }
    let used = samples.len().min(VIS_SAMPLES);
    let newest = &samples[samples.len() - used..];
    (0..width)
        .map(|x| {
            let index = x * used / width.max(1);
            newest[index.min(used - 1)].clamp(-1.0, 1.0)
        })
        .collect()
}

/// Fixed-size ring of recent mono samples written by the audio callback.
pub struct VisRing {
    samples: Vec<f32>,
    /// Index of the next slot to overwrite (the oldest sample).
    write_pos: usize,
}

impl VisRing {
    pub fn new(size: usize) -> Self {
        Self {
            samples: vec![0.0; size],
            write_pos: 0,
        }
    }

    /// Mix interleaved stereo frames down to mono and append them.
    pub fn push_stereo(&mut self, interleaved: &[f32]) {
        for frame in interleaved.chunks_exact(2) {
            self.samples[self.write_pos] = 0.5 * (frame[0] + frame[1]);
            self.write_pos = (self.write_pos + 1) % self.samples.len();
        }
    }

    pub fn clear(&mut self) {
        self.samples.fill(0.0);
    }

    /// The most recent `count` samples in chronological order.
    pub fn latest(&self, count: usize) -> Vec<f32> {
        let count = count.min(self.samples.len());
        let len = self.samples.len();
        (0..count)
            .map(|i| self.samples[(self.write_pos + len - count + i) % len])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, sample_rate: u32, count: usize) -> Vec<f32> {
        (0..count)
            .map(|i| (2.0 * PI * hz * i as f32 / sample_rate as f32).sin())
            .collect()
    }

    /// Which bar a frequency lands in, mirroring the log spacing in `spectrum`.
    fn bar_for_hz(hz: f32, bars: usize) -> usize {
        let max_hz = (44100.0f32 / 2.0).min(MAX_HZ);
        ((hz / MIN_HZ).ln() / (max_hz / MIN_HZ).ln() * bars as f32).floor() as usize
    }

    #[test]
    fn silence_gives_empty_spectrum() {
        let bars = spectrum(&[0.0; VIS_SAMPLES], 44100, 19);
        assert_eq!(bars.len(), 19);
        assert!(bars.iter().all(|&b| b == 0.0));
    }

    fn loudest_bar(bars: &[f32]) -> usize {
        bars.iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    #[test]
    fn pure_tone_peaks_in_its_own_bar() {
        // Above a few hundred Hz each bar spans at least one FFT bin. A tone
        // between two bin centres may round into the neighbouring bar, so
        // allow one bar of slack either side of the log-spacing prediction.
        for hz in [1000.0, 4000.0, 8000.0] {
            let bars = spectrum(&sine(hz, 44100, VIS_SAMPLES), 44100, 19);
            let expected = bar_for_hz(hz, 19) as i64;
            let actual = loudest_bar(&bars) as i64;
            assert!(
                (actual - expected).abs() <= 1,
                "tone at {hz} Hz: bar {actual}, expected about {expected}"
            );
        }
    }

    #[test]
    fn low_tone_peaks_near_the_left_edge() {
        // The 512-point FFT only has ~86 Hz resolution, so a bass tone can
        // only be placed roughly; it must still clearly be a bass tone.
        let bars = spectrum(&sine(100.0, 44100, VIS_SAMPLES), 44100, 19);
        assert!(loudest_bar(&bars) <= 5, "bars: {bars:?}");
    }

    #[test]
    fn spectrum_handles_fewer_samples_than_fft_size() {
        let bars = spectrum(&sine(1000.0, 44100, 100), 44100, 75);
        assert_eq!(bars.len(), 75);
        assert!(bars.iter().all(|b| (0.0..=1.0).contains(b)));
    }

    #[test]
    fn oscilloscope_downsamples_and_clamps() {
        assert_eq!(oscilloscope(&[], 4), vec![0.0; 4]);
        let points = oscilloscope(&[-3.0, 0.5, 2.0, 0.0], 2);
        assert_eq!(points, vec![-1.0, 1.0]);
        assert_eq!(oscilloscope(&[0.25], 3), vec![0.25; 3]);
    }

    #[test]
    fn vis_ring_returns_newest_samples_in_order() {
        let mut ring = VisRing::new(4);
        // Stereo frames (l, r) averaged to mono 1, 2, 3, 4, 5.
        ring.push_stereo(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0]);
        assert_eq!(ring.latest(3), vec![3.0, 4.0, 5.0]);
        assert_eq!(ring.latest(10), vec![2.0, 3.0, 4.0, 5.0]);
        ring.clear();
        assert_eq!(ring.latest(2), vec![0.0, 0.0]);
    }
}
