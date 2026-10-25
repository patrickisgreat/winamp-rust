//! Built-in procedural themes beyond the base skin.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::default::{self, Theme, BASE_THEME};
use crate::{Skin, SkinImage};

type C = [u8; 3];

pub const DMT_PLEDIT_TXT: &str = "[Text]\nNormal=#C8FF00\nCurrent=#FF4CFF\nNormalBG=#12001E\nSelectedBG=#5A1E8C\nmbFG=#C8FF00\nmbBG=#12001E\nFont=Arial\n";
pub const DMT_VISCOLOR_TXT: &str = "14,0,24\n60,10,90\n255,0,255\n240,0,255\n220,0,255\n190,20,255\n160,40,255\n120,60,255\n80,90,255\n40,140,255\n0,190,255\n0,230,230\n0,255,190\n40,255,150\n90,255,110\n140,255,80\n190,255,50\n230,255,20\n255,255,255\n255,200,255\n255,150,240\n240,110,220\n210,80,200\n255,230,0\n";

fn hsv(h: f32, s: f32, v: f32) -> C {
    let h = h.rem_euclid(360.0) / 60.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

/// Hexagonal distance from the origin (pointy-top hex metric).
fn hex_dist(x: f32, y: f32) -> f32 {
    let ax = x.abs();
    let ay = y.abs();
    (ax * 0.866_025_4 + ay * 0.5).max(ay)
}

/// Kaleidoscopic sacred-geometry pattern for the DMT theme: 8-fold mirrored
/// cells holding neon hexagonal rings, a triangle lattice and radial spokes
/// on a near-black ground, with hues cycling through the whole spectrum.
fn dmt_pattern(x: i32, y: i32, dim: bool) -> C {
    const CELL: f32 = 58.0;
    let (fx, fy) = (x as f32, y as f32);
    // Fold into one cell, then mirror in x, y and the diagonal (8-fold symmetry).
    let mut u = fx.rem_euclid(CELL) - CELL / 2.0;
    let mut v = fy.rem_euclid(CELL) - CELL / 2.0;
    u = u.abs();
    v = v.abs();
    if v > u {
        std::mem::swap(&mut u, &mut v);
    }
    let r = (u * u + v * v).sqrt();
    let a = v.atan2(u);
    let hd = hex_dist(u, v);
    let ring = hd / 5.5;
    let ring_i = ring.floor();
    let ring_f = ring - ring_i;
    let edge = (ring_f - 0.5).abs() * 2.0; // 1.0 on ring borders
    // Triangle lattice in three directions.
    let lattice = [0.0f32, 1.047_197_6, 2.094_395_2]
        .iter()
        .map(|th| {
            let d = (u * th.cos() + v * th.sin()) / 8.0;
            let f = d - d.floor();
            (f - 0.5).abs() * 2.0
        })
        .fold(0.0f32, |m, e| m.max(e));
    // Radial spokes every 30 degrees and a central star.
    let spokes = ((a * 6.0).sin()).abs();
    let star = (((a * 3.0).cos() * r / 9.0).sin()).abs();
    let hue = 290.0 + ring_i * 55.0 + (fx * 0.35) + (fy * 0.2) + r * 1.5;
    let neon = edge > 0.84 || lattice > 0.93 || (spokes < 0.06 && r > 4.0) || (star > 0.985 && r < 20.0);
    let glow = edge.max(lattice).powf(6.0) * 0.35 + (1.0 - (r / 30.0).min(1.0)) * 0.12;
    let (sat, val) = if neon {
        (0.9, if dim { 0.72 } else { 1.0 })
    } else {
        (1.0, if dim { 0.05 } else { 0.08 } + glow)
    };
    hsv(hue, sat, val)
}

pub const DMT_THEME: Theme = Theme {
    name: "DMT",
    bg: [16, 2, 28],
    bg2: [10, 1, 20],
    light: [170, 40, 255],
    lighter: [255, 110, 255],
    dark: [4, 0, 10],
    black: [0, 0, 0],
    face: [70, 12, 120],
    face_down: [36, 6, 70],
    green: [0, 255, 200],
    dim_green: [0, 115, 95],
    label: [255, 232, 120],
    label_dim: [214, 140, 255],
    white: [255, 255, 255],
    lcd: [8, 0, 18],
    accent: [255, 0, 200],
    pattern: Some(dmt_pattern),
    grad_lo: [0, 255, 190],
    grad_mid: [255, 0, 255],
    grad_hi: [255, 225, 0],
    pledit_txt: DMT_PLEDIT_TXT,
    viscolor_txt: DMT_VISCOLOR_TXT,
};

pub const LIMINAL_PLEDIT_TXT: &str = "[Text]\nNormal=#6E6248\nCurrent=#1E1A12\nNormalBG=#D9C98C\nSelectedBG=#BCAA6E\nmbFG=#6E6248\nmbBG=#D9C98C\nFont=Arial\n";
pub const LIMINAL_VISCOLOR_TXT: &str = "42,46,36\n58,62,50\n246,244,220\n240,238,210\n232,230,196\n224,222,184\n214,212,172\n204,204,160\n192,196,150\n180,188,140\n168,180,130\n156,172,122\n144,164,114\n132,156,106\n120,146,98\n108,136,90\n96,126,82\n84,116,74\n236,236,220\n220,220,204\n200,200,186\n180,182,168\n160,162,150\n250,250,230\n";

fn hash01(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Backrooms-style wallpaper: yellowed stripes under a humming fluorescent
/// glow, faint water stains, and the ghost of a drop-ceiling grid.
fn liminal_pattern(x: i32, y: i32, dim: bool) -> C {
    let (fx, fy) = (x as f32, y as f32);
    // Base wallpaper with soft vertical stripes.
    let stripe = ((fx / 6.0).sin() * 0.5 + 0.5) * 0.06 + ((fx / 23.0).cos() * 0.5 + 0.5) * 0.04;
    // Low-frequency stains.
    let stain = ((fx / 37.0).sin() * (fy / 29.0).cos() + (fx / 61.0 + fy / 17.0).sin() * 0.6) * 0.07;
    // Fluorescent light bleeding down from the ceiling.
    let glow = (1.0 - (fy / 60.0).min(1.0)) * 0.10;
    // Grain.
    let grain = (hash01(x, y) - 0.5) * 0.05;
    // Drop-ceiling grid lines every 24 px, faint.
    let grid = if x.rem_euclid(24) == 0 || y.rem_euclid(24) == 0 { -0.08 } else { 0.0 };
    let base: [f32; 3] = if dim { [0.74, 0.68, 0.46] } else { [0.85, 0.79, 0.55] };
    let k = 1.0 + stripe + stain + glow + grain + grid;
    let warm = glow * 0.6; // the light is slightly green-white
    [
        ((base[0] * k).clamp(0.0, 1.0) * 255.0) as u8,
        (((base[1] + warm * 0.3) * k).clamp(0.0, 1.0) * 255.0) as u8,
        (((base[2] + warm * 0.5) * k).clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

pub const LIMINAL_THEME: Theme = Theme {
    name: "Liminal",
    bg: [217, 201, 140],
    bg2: [190, 174, 118],
    light: [240, 232, 190],
    lighter: [250, 246, 222],
    dark: [86, 76, 50],
    black: [30, 28, 20],
    face: [208, 194, 146],
    face_down: [178, 164, 116],
    green: [228, 240, 204],
    dim_green: [118, 130, 100],
    label: [66, 56, 38],
    label_dim: [118, 104, 72],
    white: [250, 250, 236],
    lcd: [44, 48, 38],
    accent: [160, 190, 150],
    pattern: Some(liminal_pattern),
    grad_lo: [168, 188, 122],
    grad_mid: [232, 222, 146],
    grad_hi: [214, 150, 96],
    pledit_txt: LIMINAL_PLEDIT_TXT,
    viscolor_txt: LIMINAL_VISCOLOR_TXT,
};

pub const NMS_PLEDIT_TXT: &str = "[Text]\nNormal=#DCE6F0\nCurrent=#FF8A3C\nNormalBG=#121832\nSelectedBG=#1E5A78\nmbFG=#DCE6F0\nmbBG=#121832\nFont=Arial\n";
pub const NMS_VISCOLOR_TXT: &str = "10,12,30\n30,36,70\n255,80,110\n255,96,100\n255,116,88\n255,138,76\n255,160,66\n255,182,60\n250,200,70\n236,214,90\n210,222,120\n176,226,150\n140,226,180\n110,222,200\n84,216,212\n66,208,220\n56,196,226\n50,180,230\n255,255,255\n220,240,250\n180,225,240\n140,210,230\n110,195,220\n255,230,120\n";

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Distance to the nearest edge of a pointy-top hexagon grid with the given radius.
fn hex_edge_dist(x: f32, y: f32, radius: f32) -> f32 {
    let w = radius * 1.732_050_8;
    let h = radius * 1.5;
    let row = (y / h).floor();
    let xo = if (row as i32).rem_euclid(2) == 0 { 0.0 } else { w / 2.0 };
    let mut best = f32::MAX;
    for dr in -1..=1 {
        for dc in -1..=1 {
            let r = row + dr as f32;
            let off = if (r as i32).rem_euclid(2) == 0 { 0.0 } else { w / 2.0 };
            let c = ((x - off) / w).floor() + dc as f32;
            let cx = c * w + off + w / 2.0;
            let cy = r * h + radius;
            let _ = xo;
            let d = radius - hex_dist_pointy(x - cx, y - cy);
            if d.abs() < best.abs() {
                best = d;
            }
        }
    }
    best.abs()
}

fn hex_dist_pointy(x: f32, y: f32) -> f32 {
    let ax = x.abs();
    let ay = y.abs();
    (ax * 0.866_025_4 + ay * 0.5).max(ay)
}

/// No Man's Sky: nebula sky, stars, the hex interface grid and a banded planet.
fn nms_pattern(x: i32, y: i32, dim: bool) -> C {
    let (fx, fy) = (x as f32, y as f32);
    // Nebula: coral/magenta in the upper left drifting to teal/navy lower right.
    let n = ((fx / 48.0).sin() * (fy / 31.0).cos() + (fx / 23.0 + fy / 41.0).sin() * 0.6 + ((fx + fy) / 90.0).cos() * 0.8) * 0.5 + 0.5;
    let t = ((fx / 275.0) * 0.6 + (fy / 116.0) * 0.4).clamp(0.0, 1.0);
    let navy = [0.07, 0.08, 0.22];
    let purple = [0.36, 0.10, 0.42];
    let coral = [0.95, 0.38, 0.40];
    let teal = [0.10, 0.55, 0.62];
    let sky = lerp3(lerp3(coral, purple, t * 1.4), lerp3(purple, teal, (t - 0.4) * 1.7), t);
    let mut c = lerp3(navy, sky, n * 0.75);
    // Banded low-poly planet in the lower right.
    let (pcx, pcy, pr) = (296.0, 150.0, 96.0);
    let d = ((fx - pcx).powi(2) + (fy - pcy).powi(2)).sqrt();
    if d < pr {
        let band = (((fy - pcy) + (fx - pcx) * 0.35) / 9.0).floor() as i32;
        let palette = [[0.98, 0.56, 0.22], [0.92, 0.40, 0.30], [0.96, 0.72, 0.30], [0.30, 0.72, 0.70], [0.86, 0.46, 0.60]];
        let pc = palette[band.rem_euclid(palette.len() as i32) as usize];
        let shade = 0.55 + 0.45 * (1.0 - d / pr).sqrt();
        let rim = if d > pr - 2.0 { 1.3 } else { 1.0 };
        c = [pc[0] * shade * rim, pc[1] * shade * rim, pc[2] * shade * rim];
    } else if d < pr + 6.0 {
        // atmosphere glow
        let g = 1.0 - (d - pr) / 6.0;
        c = lerp3(c, [0.9, 0.6, 0.5], g * 0.6);
    } else {
        // Stars.
        let h = hash01(x, y);
        if h > 0.985 {
            let b = 0.6 + (h - 0.985) / 0.015 * 0.4;
            c = [b, b, b.min(1.0)];
        }
        if hash01(x / 2, y / 2) > 0.9985 {
            c = [1.0, 0.95, 0.85];
        }
    }
    // Hex interface grid, faint cyan.
    let e = hex_edge_dist(fx, fy, 11.0);
    if e < 0.7 {
        c = lerp3(c, [0.45, 0.85, 0.95], 0.28);
    }
    let k = if dim { 0.62 } else { 1.0 };
    [((c[0] * k).clamp(0.0, 1.0) * 255.0) as u8, ((c[1] * k).clamp(0.0, 1.0) * 255.0) as u8, ((c[2] * k).clamp(0.0, 1.0) * 255.0) as u8]
}

pub const NMS_THEME: Theme = Theme {
    name: "No Man's Sky",
    bg: [20, 24, 56],
    bg2: [12, 14, 36],
    light: [90, 175, 205],
    lighter: [160, 235, 245],
    dark: [6, 6, 20],
    black: [0, 0, 0],
    face: [255, 122, 58],
    face_down: [196, 82, 40],
    green: [120, 240, 230],
    dim_green: [50, 112, 112],
    label: [248, 242, 232],
    label_dim: [186, 206, 224],
    white: [255, 255, 255],
    lcd: [10, 12, 30],
    accent: [255, 138, 60],
    pattern: Some(nms_pattern),
    grad_lo: [60, 200, 220],
    grad_mid: [255, 180, 80],
    grad_hi: [255, 70, 90],
    pledit_txt: NMS_PLEDIT_TXT,
    viscolor_txt: NMS_VISCOLOR_TXT,
};

pub fn all() -> Vec<&'static Theme> {
    vec![&BASE_THEME, &DMT_THEME, &LIMINAL_THEME, &NMS_THEME]
}

pub fn by_name(name: &str) -> Option<&'static Theme> {
    all().into_iter().find(|t| t.name.eq_ignore_ascii_case(name) || t.name.trim_matches(['<', '>']).eq_ignore_ascii_case(name))
}

pub fn generate(theme: &Theme) -> Skin {
    default::generate_theme(theme)
}

/// Write a skin as a classic `.wsz` archive (a zip of BMPs plus the text files).
pub fn write_wsz(skin: &Skin, pledit_txt: &str, viscolor_txt: &str, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for which in SkinImage::ALL {
        let img = image::DynamicImage::ImageRgba8(skin.image(which).clone()).to_rgb8();
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Bmp)?;
        zip.start_file(format!("{}.bmp", which.file_stem()), opts)?;
        zip.write_all(bytes.get_ref())?;
    }
    zip.start_file("pledit.txt", opts)?;
    zip.write_all(pledit_txt.as_bytes())?;
    zip.start_file("viscolor.txt", opts)?;
    zip.write_all(viscolor_txt.as_bytes())?;
    zip.start_file("readme.txt", opts)?;
    zip.write_all(format!("{} - generated by winamp-rust\n", skin.name).as_bytes())?;
    zip.finish()?;
    Ok(())
}

/// Generate a theme and write it as NAME.wsz into `dir`. Returns the path.
pub fn install(theme: &Theme, dir: &Path) -> Result<std::path::PathBuf> {
    let skin = generate(theme);
    let file_name: String = theme.name.trim_matches(['<', '>']).chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    let path = dir.join(format!("{}.wsz", file_name.replace(' ', "")));
    write_wsz(&skin, theme.pledit_txt, theme.viscolor_txt, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmt_wsz_roundtrip() {
        let dir = std::env::temp_dir().join("winamp-rust-test-skins");
        let path = install(&DMT_THEME, &dir).unwrap();
        let skin = Skin::load(&path).unwrap();
        assert_eq!(skin.image(SkinImage::Main).dimensions(), (275, 116));
        assert!(skin.provided.len() == SkinImage::ALL.len());
        assert_eq!(skin.pledit.normal, crate::Rgb(0xC8, 0xFF, 0x00));
        assert_eq!(skin.viscolors.0[2], crate::Rgb(255, 0, 255));
    }
}
