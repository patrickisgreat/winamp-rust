//! Visualization analysis: spectrum analyser and oscilloscope sampling.

use std::f32::consts::PI;

/// Number of samples Winamp hands to visualisation plugins per frame.
pub const VIS_SAMPLES: usize = 576;
const FFT_SIZE: usize = 512;

/// Simple in-place radix-2 FFT (real input packed as complex).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
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
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (xr, xi) = (re[i + k + len / 2], im[i + k + len / 2]);
                let vr = xr * cr - xi * ci;
                let vi = xr * ci + xi * cr;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + len / 2] = ur - vr;
                im[i + k + len / 2] = ui - vi;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Compute `bars` log-spaced spectrum magnitudes (0.0..=1.0) from mono samples.
pub fn spectrum(samples: &[f32], sample_rate: u32, bars: usize) -> Vec<f32> {
    let mut re = vec![0.0f32; FFT_SIZE];
    let mut im = vec![0.0f32; FFT_SIZE];
    let n = samples.len().min(FFT_SIZE);
    for i in 0..n {
        // Hann window
        let w = 0.5 - 0.5 * (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos();
        re[i] = samples[samples.len() - n + i] * w;
    }
    fft(&mut re, &mut im);
    let bins = FFT_SIZE / 2;
    let mut mags = vec![0.0f32; bins];
    for i in 0..bins {
        mags[i] = (re[i] * re[i] + im[i] * im[i]).sqrt() / (FFT_SIZE as f32 / 4.0);
    }
    let sr = sample_rate.max(8000) as f32;
    let bin_hz = sr / FFT_SIZE as f32;
    let f_min = 40.0f32;
    let f_max = (sr / 2.0).min(20000.0);
    let mut out = Vec::with_capacity(bars);
    for b in 0..bars {
        let t0 = b as f32 / bars as f32;
        let t1 = (b + 1) as f32 / bars as f32;
        let lo = f_min * (f_max / f_min).powf(t0);
        let hi = f_min * (f_max / f_min).powf(t1);
        let mut b0 = (lo / bin_hz) as usize;
        let mut b1 = (hi / bin_hz) as usize;
        b0 = b0.clamp(1, bins - 1);
        b1 = b1.clamp(b0 + 1, bins);
        let mut peak = 0.0f32;
        for m in &mags[b0..b1] {
            peak = peak.max(*m);
        }
        // Convert to dB-ish scale that looks like Winamp's analyser.
        let db = 20.0 * (peak + 1e-6).log10();
        let v = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        out.push(v);
    }
    out
}

/// Downsample mono audio to `width` oscilloscope points in -1.0..=1.0.
pub fn oscilloscope(samples: &[f32], width: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; width];
    }
    let n = samples.len().min(VIS_SAMPLES);
    let slice = &samples[samples.len() - n..];
    (0..width)
        .map(|i| {
            let idx = i * n / width.max(1);
            slice[idx.min(n - 1)].clamp(-1.0, 1.0)
        })
        .collect()
}

/// Fixed-size ring of recent mono samples written by the audio callback.
pub struct VisRing {
    buf: Vec<f32>,
    pos: usize,
}

impl VisRing {
    pub fn new(size: usize) -> Self {
        Self { buf: vec![0.0; size], pos: 0 }
    }

    pub fn push_stereo(&mut self, interleaved: &[f32]) {
        for frame in interleaved.chunks_exact(2) {
            self.buf[self.pos] = 0.5 * (frame[0] + frame[1]);
            self.pos = (self.pos + 1) % self.buf.len();
        }
    }

    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = 0.0);
    }

    /// The most recent `n` samples in chronological order.
    pub fn latest(&self, n: usize) -> Vec<f32> {
        let n = n.min(self.buf.len());
        let len = self.buf.len();
        (0..n).map(|i| self.buf[(self.pos + len - n + i) % len]).collect()
    }
}
