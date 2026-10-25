//! Render the application icon (a lightning bolt on a dark rounded tile):
//! `cargo run -p winamp-skin --example icon -- out.png [size]`
use image::{Rgba, RgbaImage};

fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "icon.png".into());
    let size: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    let s = size as f32;
    let mut img = RgbaImage::new(size, size);
    let radius = s * 0.22;
    // Lightning bolt in unit coordinates.
    let bolt: Vec<(f32, f32)> = [
        (0.58, 0.12),
        (0.30, 0.54),
        (0.47, 0.54),
        (0.40, 0.88),
        (0.70, 0.44),
        (0.53, 0.44),
        (0.62, 0.12),
    ]
    .iter()
    .map(|(x, y)| (x * s, y * s))
    .collect();
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded square mask.
            let dx = (fx - s / 2.0).abs() - (s / 2.0 - radius - s * 0.04);
            let dy = (fy - s / 2.0).abs() - (s / 2.0 - radius - s * 0.04);
            let d = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() - radius;
            let aa = (1.0 - d / 2.0).clamp(0.0, 1.0);
            if aa <= 0.0 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }
            // Background gradient: deep navy to purple.
            let t = fy / s;
            let bg = [
                (24.0 + 40.0 * t) as u8,
                (26.0 + 10.0 * t) as u8,
                (66.0 + 60.0 * t) as u8,
            ];
            let mut c = bg;
            // Soft orange glow behind the bolt.
            let gd = ((fx - s * 0.5).powi(2) + (fy - s * 0.5).powi(2)).sqrt() / (s * 0.45);
            let glow = (1.0 - gd).clamp(0.0, 1.0).powi(2) * 0.45;
            c = [
                (c[0] as f32 + (255.0 - c[0] as f32) * glow) as u8,
                (c[1] as f32 + (140.0 - c[1] as f32) * glow) as u8,
                (c[2] as f32 + (60.0 - c[2] as f32) * glow) as u8,
            ];
            // Bolt with a dark outline.
            let outline = (-2..=2).any(|ox| {
                (-2..=2).any(|oy| {
                    inside(
                        &bolt,
                        fx + ox as f32 * s * 0.008,
                        fy + oy as f32 * s * 0.008,
                    )
                })
            });
            if outline {
                c = [20, 10, 30];
            }
            if inside(&bolt, fx, fy) {
                let k = 1.0 - (fy / s - 0.12) / 0.76;
                c = [255, (150.0 + 90.0 * k) as u8, (40.0 + 40.0 * k) as u8];
            }
            img.put_pixel(x, y, Rgba([c[0], c[1], c[2], (aa * 255.0) as u8]));
        }
    }
    img.save(&out).expect("save icon");
    println!("wrote {out}");
}
