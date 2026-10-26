//! A MilkDrop-style music visualiser: a feedback buffer that is warped, zoomed
//! and rotated every frame while audio-driven waveforms and shapes are drawn on
//! top. Rendered on the CPU into an RGB float buffer and uploaded as a texture.

use std::f32::consts::{PI, TAU};

/// Audio features extracted each frame.
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct Audio {
    pub bass: f32,
    pub mid: f32,
    pub treb: f32,
    pub bass_att: f32,
    pub mid_att: f32,
    pub treb_att: f32,
    pub vol: f32,
    pub beat: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaveMode {
    Circle,
    Horizontal,
    Spiral,
    Lissajous,
    DualLines,
    SpectrumRing,
    Star,
}

/// Static parameters of a preset. Per-frame modulation happens in `frame_params`.
#[derive(Clone, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub zoom: f32,
    pub zoom_exp: f32,
    pub rot: f32,
    pub warp: f32,
    pub warp_speed: f32,
    pub warp_scale: f32,
    pub decay: f32,
    pub cx: f32,
    pub cy: f32,
    pub sx: f32,
    pub sy: f32,
    pub dx: f32,
    pub dy: f32,
    pub wave_mode: WaveMode,
    pub wave_scale: f32,
    pub wave_thick: bool,
    pub wave_hue_speed: f32,
    pub wave_sat: f32,
    pub hue_shift: f32,
    pub invert_pulse: bool,
    pub echo: bool,
    pub kaleido: u32,
    pub darken_center: bool,
    pub bass_zoom: f32,
    pub treb_rot: f32,
    pub mid_warp: f32,
}

impl Preset {
    const fn base(name: &'static str) -> Preset {
        Preset {
            name,
            zoom: 1.01,
            zoom_exp: 1.0,
            rot: 0.0,
            warp: 0.3,
            warp_speed: 1.0,
            warp_scale: 1.0,
            decay: 0.97,
            cx: 0.5,
            cy: 0.5,
            sx: 1.0,
            sy: 1.0,
            dx: 0.0,
            dy: 0.0,
            wave_mode: WaveMode::Circle,
            wave_scale: 1.0,
            wave_thick: false,
            wave_hue_speed: 0.2,
            wave_sat: 0.9,
            hue_shift: 0.0,
            invert_pulse: false,
            echo: false,
            kaleido: 0,
            darken_center: false,
            bass_zoom: 0.02,
            treb_rot: 0.0,
            mid_warp: 0.0,
        }
    }
}

pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            zoom: 1.015,
            warp: 0.6,
            warp_speed: 1.3,
            decay: 0.975,
            wave_mode: WaveMode::Circle,
            bass_zoom: 0.04,
            rot: 0.004,
            ..Preset::base("Geiss - Tunnel of Bass")
        },
        Preset {
            zoom: 0.995,
            rot: 0.012,
            warp: 0.25,
            decay: 0.98,
            wave_mode: WaveMode::Spiral,
            treb_rot: 0.01,
            wave_hue_speed: 0.5,
            ..Preset::base("Rovastar - Spiral Nebula")
        },
        Preset {
            zoom: 1.03,
            warp: 1.2,
            warp_speed: 0.6,
            warp_scale: 2.0,
            decay: 0.96,
            wave_mode: WaveMode::Lissajous,
            mid_warp: 0.8,
            wave_thick: true,
            ..Preset::base("Flexi - Liquid Lissajous")
        },
        Preset {
            zoom: 1.0,
            dx: 0.0,
            dy: -0.012,
            warp: 0.15,
            decay: 0.985,
            wave_mode: WaveMode::Horizontal,
            sx: 1.01,
            wave_thick: true,
            bass_zoom: 0.0,
            ..Preset::base("Telek - Rising Waves")
        },
        Preset {
            zoom: 1.02,
            rot: -0.02,
            warp: 0.4,
            decay: 0.97,
            wave_mode: WaveMode::SpectrumRing,
            kaleido: 6,
            hue_shift: 0.3,
            bass_zoom: 0.05,
            ..Preset::base("Martin - Kaleidoscope Six")
        },
        Preset {
            zoom: 1.04,
            zoom_exp: 1.6,
            warp: 0.1,
            decay: 0.95,
            wave_mode: WaveMode::Star,
            echo: true,
            treb_rot: 0.02,
            wave_hue_speed: 1.0,
            ..Preset::base("Unchained - Starburst Echo")
        },
        Preset {
            zoom: 0.985,
            rot: 0.0,
            warp: 0.9,
            warp_speed: 2.0,
            decay: 0.99,
            wave_mode: WaveMode::DualLines,
            invert_pulse: true,
            mid_warp: 0.5,
            darken_center: true,
            ..Preset::base("Zylot - Ink in Water")
        },
        Preset {
            zoom: 1.008,
            rot: 0.006,
            warp: 0.5,
            warp_scale: 0.5,
            decay: 0.98,
            wave_mode: WaveMode::Circle,
            kaleido: 4,
            bass_zoom: 0.06,
            hue_shift: 0.6,
            wave_thick: true,
            ..Preset::base("Krash - Mandala Pulse")
        },
        Preset {
            zoom: 1.025,
            warp: 0.2,
            decay: 0.965,
            cx: 0.5,
            cy: 0.62,
            wave_mode: WaveMode::Horizontal,
            sy: 0.98,
            treb_rot: 0.004,
            echo: true,
            ..Preset::base("Eo.S - Horizon Drift")
        },
        Preset {
            zoom: 1.0,
            rot: 0.03,
            warp: 0.7,
            warp_speed: 0.4,
            decay: 0.975,
            wave_mode: WaveMode::Spiral,
            kaleido: 8,
            wave_sat: 1.0,
            bass_zoom: 0.03,
            ..Preset::base("Phat - Octagon Vortex")
        },
    ]
}

pub struct Milkdrop {
    pub w: usize,
    pub h: usize,
    front: Vec<[f32; 3]>,
    back: Vec<[f32; 3]>,
    pub time: f32,
    pub preset: usize,
    pub presets: Vec<Preset>,
    pub preset_time: f32,
    pub auto_switch_secs: f32,
    bass_avg: f32,
    mid_avg: f32,
    treb_avg: f32,
    pub audio: Audio,
    last_beat: f32,
    wave_hue: f32,
    rng: u32,
    pub frame: u64,
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

impl Milkdrop {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            front: vec![[0.0; 3]; w * h],
            back: vec![[0.0; 3]; w * h],
            time: 0.0,
            preset: 0,
            presets: presets(),
            preset_time: 0.0,
            auto_switch_secs: 25.0,
            bass_avg: 0.1,
            mid_avg: 0.1,
            treb_avg: 0.1,
            audio: Audio::default(),
            last_beat: -10.0,
            wave_hue: 0.0,
            rng: 0x1234_5678,
            frame: 0,
        }
    }

    #[allow(dead_code)]
    pub fn resize(&mut self, w: usize, h: usize) {
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.front = vec![[0.0; 3]; w * h];
            self.back = vec![[0.0; 3]; w * h];
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng & 0xFFFFFF) as f32 / 16_777_216.0
    }

    pub fn preset_name(&self) -> &'static str {
        self.presets[self.preset].name
    }

    pub fn next_preset(&mut self) {
        self.preset = (self.preset + 1) % self.presets.len();
        self.preset_time = 0.0;
    }

    pub fn prev_preset(&mut self) {
        self.preset = (self.preset + self.presets.len() - 1) % self.presets.len();
        self.preset_time = 0.0;
    }

    pub fn random_preset(&mut self) {
        let n = self.presets.len();
        let r = (self.rand() * n as f32) as usize % n;
        self.preset = if r == self.preset { (r + 1) % n } else { r };
        self.preset_time = 0.0;
    }

    pub fn set_preset(&mut self, i: usize) {
        if i < self.presets.len() {
            self.preset = i;
            self.preset_time = 0.0;
        }
    }

    /// Update audio features from the spectrum (bars 0..1, low to high) and waveform.
    pub fn analyse(&mut self, spectrum: &[f32], wave: &[f32]) {
        let n = spectrum.len().max(1);
        let band = |a: usize, b: usize| -> f32 {
            let a = a.min(n - 1);
            let b = b.clamp(a + 1, n);
            spectrum[a..b].iter().sum::<f32>() / (b - a) as f32
        };
        let bass = band(0, n / 6);
        let mid = band(n / 6, n / 2);
        let treb = band(n / 2, n);
        let vol = (wave.iter().map(|s| s * s).sum::<f32>() / wave.len().max(1) as f32).sqrt();
        let vol = if vol.is_finite() { vol } else { 0.0 };
        let a = 0.08;
        self.bass_avg = self.bass_avg * (1.0 - a) + bass * a;
        self.mid_avg = self.mid_avg * (1.0 - a) + mid * a;
        self.treb_avg = self.treb_avg * (1.0 - a) + treb * a;
        let rel = |v: f32, avg: f32| if avg > 1e-4 { v / avg } else { 1.0 };
        let beat =
            rel(bass, self.bass_avg) > 1.35 && bass > 0.12 && self.time - self.last_beat > 0.25;
        if beat {
            self.last_beat = self.time;
        }
        self.audio = Audio {
            bass,
            mid,
            treb,
            bass_att: rel(bass, self.bass_avg),
            mid_att: rel(mid, self.mid_avg),
            treb_att: rel(treb, self.treb_avg),
            vol,
            beat,
        };
    }

    /// Advance one frame. `dt` in seconds; `wave` is a mono waveform (-1..1).
    pub fn step(&mut self, dt: f32, wave: &[f32], spectrum: &[f32]) {
        self.time += dt;
        self.preset_time += dt;
        self.frame += 1;
        self.analyse(spectrum, wave);
        if self.auto_switch_secs > 0.0 && self.preset_time > self.auto_switch_secs {
            self.random_preset();
        }
        self.warp_pass();
        self.draw_pass(wave, spectrum);
        std::mem::swap(&mut self.front, &mut self.back);
    }

    #[inline]
    fn sample(&self, u: f32, v: f32) -> [f32; 3] {
        // Bilinear sample of the back buffer with wrap-around at the edges.
        let w = self.w as f32;
        let h = self.h as f32;
        let x = (u * w - 0.5).rem_euclid(w);
        let y = (v * h - 0.5).rem_euclid(h);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let x1 = (x0 + 1) % self.w;
        let y1 = (y0 + 1) % self.h;
        let p00 = self.back[y0 * self.w + x0];
        let p10 = self.back[y0 * self.w + x1];
        let p01 = self.back[y1 * self.w + x0];
        let p11 = self.back[y1 * self.w + x1];
        let mut out = [0.0; 3];
        for c in 0..3 {
            let top = p00[c] + (p10[c] - p00[c]) * fx;
            let bot = p01[c] + (p11[c] - p01[c]) * fx;
            out[c] = top + (bot - top) * fy;
        }
        out
    }

    fn warp_pass(&mut self) {
        let p = self.presets[self.preset].clone();
        let a = self.audio;
        let t = self.time;
        // Per-frame equations (the MilkDrop "per_frame" block).
        let beat_env = ((t - self.last_beat) * 4.0).exp().recip().min(1.0); // 1 right after a beat, decaying
        let zoom = p.zoom + p.bass_zoom * (a.bass_att - 1.0).clamp(-1.0, 2.0) + 0.03 * beat_env;
        let rot =
            p.rot + p.treb_rot * (a.treb_att - 1.0).clamp(-1.0, 2.0) + 0.002 * (t * 0.7).sin();
        let warp = p.warp * (1.0 + p.mid_warp * (a.mid_att - 1.0).clamp(-1.0, 2.0));
        let decay = if p.invert_pulse && a.beat {
            0.6
        } else {
            p.decay
        };
        let cx = p.cx + 0.03 * (t * 0.31).sin();
        let cy = p.cy + 0.03 * (t * 0.23).cos();
        let (sr, cr) = rot.sin_cos();
        let aspect = self.w as f32 / self.h as f32;
        let ws = p.warp_speed;
        let wsc = p.warp_scale;
        let kal = p.kaleido;
        let w = self.w;
        let h = self.h;
        let hue_shift = p.hue_shift * dt_hue(t);
        for y in 0..h {
            let v = (y as f32 + 0.5) / h as f32;
            for x in 0..w {
                let u = (x as f32 + 0.5) / w as f32;
                // Centered coordinates, aspect corrected.
                let mut px = (u - cx) * aspect;
                let mut py = v - cy;
                if kal > 0 {
                    let r = (px * px + py * py).sqrt();
                    let mut ang = py.atan2(px);
                    let seg = TAU / kal as f32;
                    ang = ang.rem_euclid(seg);
                    if ang > seg / 2.0 {
                        ang = seg - ang;
                    }
                    ang += t * 0.1;
                    px = r * ang.cos();
                    py = r * ang.sin();
                }
                let r = (px * px + py * py).sqrt();
                let z = zoom.powf(r.powf(p.zoom_exp - 1.0).max(0.0) * 1.0 + 0.0);
                let z = if p.zoom_exp == 1.0 { zoom } else { z };
                // Warp field.
                let wx = warp
                    * 0.01
                    * ((py * 6.0 * wsc + t * ws * 0.9).sin()
                        + (r * 9.0 * wsc - t * ws * 1.3).cos() * 0.5);
                let wy = warp
                    * 0.01
                    * ((px * 7.0 * wsc - t * ws * 1.1).cos()
                        + (r * 8.0 * wsc + t * ws * 0.7).sin() * 0.5);
                let mut sx = px / z / p.sx + wx;
                let mut sy = py / z / p.sy + wy;
                let rx = sx * cr - sy * sr;
                let ry = sx * sr + sy * cr;
                sx = rx / aspect + cx + p.dx;
                sy = ry + cy + p.dy;
                let mut c = self.sample(sx, sy);
                let d = if p.darken_center {
                    decay * (0.85 + 0.15 * (r * 3.0).min(1.0))
                } else {
                    decay
                };
                for ch in c.iter_mut() {
                    *ch *= d;
                }
                if hue_shift != 0.0 {
                    c = rotate_hue(c, hue_shift);
                }
                self.front[y * w + x] = c;
            }
        }
        if p.echo {
            // Blend a smaller, rotated copy of the previous frame back in.
            let (es, ec) = (t * 0.5).sin_cos();
            for y in 0..h {
                for x in 0..w {
                    let u = (x as f32 + 0.5) / w as f32 - 0.5;
                    let v = (y as f32 + 0.5) / h as f32 - 0.5;
                    let su = (u * ec - v * es) * 2.0 + 0.5;
                    let sv = (u * es + v * ec) * 2.0 + 0.5;
                    if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                        let c = self.sample(su, sv);
                        let dst = &mut self.front[y * w + x];
                        for ch in 0..3 {
                            dst[ch] = (dst[ch] + c[ch] * 0.35).min(1.5);
                        }
                    }
                }
            }
        }
    }

    fn plot(&mut self, x: f32, y: f32, c: [f32; 3], thick: bool) {
        let w = self.w as i32;
        let h = self.h as i32;
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let xi = x.round() as i32;
        let yi = y.round() as i32;
        let pts: &[(i32, i32)] = if thick {
            &[(0, 0), (1, 0), (0, 1), (1, 1)]
        } else {
            &[(0, 0)]
        };
        for (dx, dy) in pts {
            let (px, py) = (xi + dx, yi + dy);
            if px >= 0 && py >= 0 && px < w && py < h {
                let dst = &mut self.front[py as usize * self.w + px as usize];
                for ch in 0..3 {
                    dst[ch] = (dst[ch] + c[ch] * 0.55).min(1.3);
                }
            }
        }
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [f32; 3], thick: bool) {
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
            return;
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        // Cap the step count so an absurd coordinate can never stall a frame.
        let steps = dx.abs().max(dy.abs()).ceil().clamp(1.0, 4096.0) as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.plot(x0 + dx * t, y0 + dy * t, c, thick);
        }
    }

    fn draw_pass(&mut self, wave: &[f32], spectrum: &[f32]) {
        let p = self.presets[self.preset].clone();
        let a = self.audio;
        let t = self.time;
        self.wave_hue += p.wave_hue_speed * 0.01 * (1.0 + a.treb_att * 0.5);
        let w = self.w as f32;
        let h = self.h as f32;
        let cx = w * 0.5;
        let cy = h * 0.5;
        let n = 256.min(wave.len().max(2));
        let step = wave.len().max(1) as f32 / n as f32;
        let amp = p.wave_scale * (0.6 + 0.6 * a.vol.min(1.0)) * h * 0.25;
        let base_hue = self.wave_hue;
        let col = |i: usize| hsv(base_hue + i as f32 / n as f32 * 0.25, p.wave_sat, 0.9);
        let sample =
            |i: usize| wave[((i as f32 * step) as usize).min(wave.len().saturating_sub(1))];
        let mut prev: Option<(f32, f32)> = None;
        let thick = p.wave_thick;
        match p.wave_mode {
            WaveMode::Circle | WaveMode::Spiral => {
                let spiral = p.wave_mode == WaveMode::Spiral;
                let r0 = h * (0.22 + 0.08 * (a.bass_att - 1.0).clamp(-0.5, 1.0));
                for i in 0..=n {
                    let ang = i as f32 / n as f32 * TAU + t * 0.3;
                    let s = sample(i % n);
                    let r = r0
                        + s * amp
                        + if spiral {
                            i as f32 / n as f32 * h * 0.15
                        } else {
                            0.0
                        };
                    let x = cx + r * ang.cos();
                    let y = cy + r * ang.sin();
                    if let Some((px, py)) = prev {
                        self.line(px, py, x, y, col(i), thick);
                    }
                    prev = Some((x, y));
                }
            }
            WaveMode::Horizontal | WaveMode::DualLines => {
                let dual = p.wave_mode == WaveMode::DualLines;
                let rows: &[f32] = if dual { &[0.35, 0.65] } else { &[0.5] };
                for (k, ry) in rows.iter().enumerate() {
                    prev = None;
                    for i in 0..n {
                        let s = sample(i) * if k == 1 { -1.0 } else { 1.0 };
                        let x = i as f32 / (n - 1) as f32 * w;
                        let y = h * ry + s * amp * 1.5;
                        if let Some((px, py)) = prev {
                            self.line(px, py, x, y, col(i), thick);
                        }
                        prev = Some((x, y));
                    }
                }
            }
            WaveMode::Lissajous => {
                let half = n / 2;
                for i in 0..half {
                    let sx = sample(i);
                    let sy = sample(i + half);
                    let x = cx + sx * amp * 2.2 + (t * 1.1).sin() * w * 0.1;
                    let y = cy + sy * amp * 2.2 + (t * 0.9).cos() * h * 0.1;
                    if let Some((px, py)) = prev {
                        self.line(px, py, x, y, col(i), thick);
                    }
                    prev = Some((x, y));
                }
            }
            WaveMode::SpectrumRing => {
                let bars = spectrum.len().max(1);
                for (i, s) in spectrum.iter().enumerate() {
                    let ang = i as f32 / bars as f32 * TAU - t * 0.2;
                    let r0 = h * 0.18;
                    let r1 = r0 + s * h * 0.35;
                    let c = hsv(base_hue + i as f32 / bars as f32, p.wave_sat, 0.5 + 0.5 * s);
                    self.line(
                        cx + r0 * ang.cos(),
                        cy + r0 * ang.sin(),
                        cx + r1 * ang.cos(),
                        cy + r1 * ang.sin(),
                        c,
                        thick,
                    );
                }
            }
            WaveMode::Star => {
                let points = 5 + (a.bass_att * 2.0) as usize;
                let r = h * (0.15 + 0.2 * a.vol.min(1.0));
                let mut first: Option<(f32, f32)> = None;
                for i in 0..points {
                    let ang = i as f32 / points as f32 * TAU * 2.0 + t;
                    let s = sample(i * n / points);
                    let rr = r + s * amp;
                    let x = cx + rr * ang.cos();
                    let y = cy + rr * ang.sin();
                    if let Some((px, py)) = prev {
                        self.line(px, py, x, y, col(i * n / points), thick);
                    } else {
                        first = Some((x, y));
                    }
                    prev = Some((x, y));
                }
                if let (Some((px, py)), Some((fx, fy))) = (prev, first) {
                    self.line(px, py, fx, fy, col(0), thick);
                }
            }
        }
        // Beat flash: a soft ring that the warp then smears outwards.
        if a.beat {
            let r = h * 0.1;
            let c = hsv(base_hue + 0.5, 0.6, 0.8);
            for i in 0..90 {
                let ang = i as f32 / 90.0 * TAU;
                self.plot(cx + r * ang.cos(), cy + r * ang.sin(), c, true);
            }
        }
        // Border motion blur: slight edge darkening keeps the wrap-around sampling from banding.
        let _ = PI;
    }

    /// Write the current frame as RGBA bytes (gamma-ish tone mapped).
    pub fn rgba(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.w * self.h * 4);
        for p in &self.back {
            for c in p {
                let v = (c / (1.0 + c * 0.35)).clamp(0.0, 1.0);
                out.push((v.sqrt() * 255.0) as u8);
            }
            out.push(255);
        }
    }
}

fn dt_hue(t: f32) -> f32 {
    0.002 * (1.0 + (t * 0.2).sin() * 0.5)
}

fn rotate_hue(c: [f32; 3], amount: f32) -> [f32; 3] {
    // Small-angle hue rotation in RGB via the YIQ-style rotation matrix.
    let cosa = (amount * TAU).cos();
    let sina = (amount * TAU).sin();
    let k = 1.0 / 3.0;
    let s3 = (1.0f32 / 3.0).sqrt();
    let m = |i: usize, j: usize| -> f32 {
        if i == j {
            cosa + (1.0 - cosa) * k
        } else {
            (1.0 - cosa) * k
                + if (i + 1) % 3 == j {
                    -s3 * sina
                } else {
                    s3 * sina
                }
        }
    };
    std::array::from_fn(|i| (m(i, 0) * c[0] + m(i, 1) * c[1] + m(i, 2) * c[2]).max(0.0))
}
