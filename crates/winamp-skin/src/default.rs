//! Procedurally generated default skin. This keeps the application free of
//! Nullsoft's copyrighted bitmaps while still exercising the exact sprite
//! layout that real classic skins use.

use std::collections::HashMap;

use image::{Rgba, RgbaImage};

use crate::font;
use crate::{PleditColors, Skin, SkinImage, VisColors};

pub const PLEDIT_TXT: &str =
    "[Text]\nNormal=#00FF00\nCurrent=#FFFFFF\nNormalBG=#000000\nSelectedBG=#0000C6\nFont=Arial\n";
pub const VISCOLOR_TXT: &str = "0,0,0\n24,33,41\n239,49,16\n206,41,16\n214,90,0\n214,102,0\n214,115,0\n198,123,8\n222,165,24\n214,181,33\n189,222,41\n148,222,33\n41,206,16\n50,190,16\n57,181,16\n49,156,8\n41,148,0\n24,132,8\n255,255,255\n214,214,222\n181,189,189\n160,170,175\n148,156,165\n150,150,150\n";

type C = [u8; 3];

/// Colors and background pattern used by the procedural skin generator.
#[derive(Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub bg: C,
    pub bg2: C,
    pub light: C,
    pub lighter: C,
    pub dark: C,
    pub black: C,
    pub face: C,
    pub face_down: C,
    pub green: C,
    pub dim_green: C,
    pub label: C,
    pub label_dim: C,
    pub white: C,
    pub lcd: C,
    pub accent: C,
    /// Optional background pattern: (x, y, dim) -> color.
    pub pattern: Option<fn(i32, i32, bool) -> C>,
    /// Gradient end points for slider fills (low .. high).
    pub grad_lo: C,
    pub grad_mid: C,
    pub grad_hi: C,
    pub pledit_txt: &'static str,
    pub viscolor_txt: &'static str,
    /// Title bar texts for the main, equalizer and playlist windows.
    pub title_main: &'static str,
    pub title_eq: &'static str,
    pub title_pl: &'static str,
    /// Title text color.
    pub title_color: C,
    /// Optional 9x9 emblem drawn left of the title text.
    pub emblem: Option<fn(&mut Canvas, i32, i32)>,
    /// Flat "HUD" widget style: translucent chamfered panels instead of bevels.
    pub flat: bool,
    /// Extra artwork drawn over a finished bitmap.
    pub decorate: Option<fn(&mut Canvas, SkinImage)>,
}

pub const BASE_THEME: Theme = Theme {
    name: "<Base Skin>",
    bg: [54, 58, 76],
    bg2: [46, 49, 66],
    light: [104, 110, 140],
    lighter: [140, 146, 176],
    dark: [24, 25, 36],
    black: [0, 0, 0],
    face: [70, 75, 98],
    face_down: [40, 43, 58],
    green: [0, 255, 0],
    dim_green: [0, 104, 0],
    label: [214, 218, 232],
    label_dim: [120, 126, 150],
    white: [255, 255, 255],
    lcd: [0, 0, 0],
    accent: [84, 160, 255],
    pattern: None,
    grad_lo: [0, 200, 0],
    grad_mid: [230, 230, 0],
    grad_hi: [240, 40, 0],
    pledit_txt: PLEDIT_TXT,
    viscolor_txt: VISCOLOR_TXT,
    title_main: "WINAMP",
    title_eq: "WINAMP EQUALIZER",
    title_pl: "PLAYLIST EDITOR",
    title_color: [214, 218, 232],
    emblem: None,
    flat: false,
    decorate: None,
};

/// Tiny 3x5 font for small labels.
fn mini_glyph(c: char) -> [&'static str; 5] {
    match c.to_ascii_uppercase() {
        'A' => [".#.", "#.#", "###", "#.#", "#.#"],
        'B' => ["##.", "#.#", "##.", "#.#", "##."],
        'C' => ["###", "#..", "#..", "#..", "###"],
        'D' => ["##.", "#.#", "#.#", "#.#", "##."],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'F' => ["###", "#..", "##.", "#..", "#.."],
        'G' => ["###", "#..", "#.#", "#.#", "###"],
        'H' => ["#.#", "#.#", "###", "#.#", "#.#"],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'J' => ["..#", "..#", "..#", "#.#", "###"],
        'K' => ["#.#", "#.#", "##.", "#.#", "#.#"],
        'L' => ["#..", "#..", "#..", "#..", "###"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'N' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'O' => ["###", "#.#", "#.#", "#.#", "###"],
        'P' => ["###", "#.#", "###", "#..", "#.."],
        'Q' => ["###", "#.#", "#.#", "###", "..#"],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'S' => ["###", "#..", "###", "..#", "###"],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'V' => ["#.#", "#.#", "#.#", "#.#", ".#."],
        'W' => ["#.#", "#.#", "###", "###", "#.#"],
        'X' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
        'Y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
        'Z' => ["###", "..#", ".#.", "#..", "###"],
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["###", "..#", "###", "#..", "###"],
        '3' => ["###", "..#", "###", "..#", "###"],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "###", "..#", "###"],
        '6' => ["###", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", "..#", "..#", "..#"],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "###"],
        '-' => ["...", "...", "###", "...", "..."],
        '+' => ["...", ".#.", "###", ".#.", "..."],
        '.' => ["...", "...", "...", "...", ".#."],
        '/' => ["..#", "..#", ".#.", "#..", "#.."],
        ':' => ["...", ".#.", "...", ".#.", "..."],
        _ => ["...", "...", "...", "...", "..."],
    }
}

pub struct Canvas {
    img: RgbaImage,
    t: Theme,
    /// Offset added to pattern coordinates so sprites continue the window artwork.
    pub origin: (i32, i32),
}

impl Canvas {
    fn new(t: &Theme, w: u32, h: u32, c: C) -> Self {
        let mut img = RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = Rgba([c[0], c[1], c[2], 255]);
        }
        let mut cv = Self {
            img,
            t: *t,
            origin: (0, 0),
        };
        if c == t.bg && t.pattern.is_some() {
            cv.fill_bg(0, 0, w as i32, h as i32, false);
        }
        cv
    }

    /// Fill with the themed background (pattern if the theme has one).
    fn fill_bg(&mut self, x: i32, y: i32, w: i32, h: i32, dim: bool) {
        match self.t.pattern {
            Some(f) => {
                let (ox, oy) = self.origin;
                for yy in y..y + h {
                    for xx in x..x + w {
                        self.px(xx, yy, f(xx + ox, yy + oy, dim));
                    }
                }
            }
            None => self.fill(x, y, w, h, if dim { self.t.bg2 } else { self.t.bg }),
        }
    }

    pub fn px(&mut self, x: i32, y: i32, c: C) {
        if x >= 0 && y >= 0 && (x as u32) < self.img.width() && (y as u32) < self.img.height() {
            self.img
                .put_pixel(x as u32, y as u32, Rgba([c[0], c[1], c[2], 255]));
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: C) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
    }

    pub fn hline(&mut self, x: i32, y: i32, w: i32, c: C) {
        self.fill(x, y, w, 1, c);
    }

    pub fn vline(&mut self, x: i32, y: i32, h: i32, c: C) {
        self.fill(x, y, 1, h, c);
    }

    pub fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: C) {
        self.hline(x, y, w, c);
        self.hline(x, y + h - 1, w, c);
        self.vline(x, y, h, c);
        self.vline(x + w - 1, y, h, c);
    }

    /// Multiply the existing pixels in a rectangle by `k` (translucent panels).
    pub fn darken(&mut self, x: i32, y: i32, w: i32, h: i32, k: f32) {
        for yy in y.max(0)..(y + h).min(self.img.height() as i32) {
            for xx in x.max(0)..(x + w).min(self.img.width() as i32) {
                let p = self.img.get_pixel(xx as u32, yy as u32);
                let c = [
                    (p[0] as f32 * k) as u8,
                    (p[1] as f32 * k) as u8,
                    (p[2] as f32 * k) as u8,
                ];
                self.px(xx, yy, c);
            }
        }
    }

    /// Chamfered outline (corner pixels skipped).
    pub fn chamfer_outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: C) {
        self.hline(x + 1, y, w - 2, c);
        self.hline(x + 1, y + h - 1, w - 2, c);
        self.vline(x, y + 1, h - 2, c);
        self.vline(x + w - 1, y + 1, h - 2, c);
    }

    /// Scaled-up mini font text (each pixel becomes `k` x `k`).
    pub fn text3_scaled(&mut self, x: i32, y: i32, s: &str, c: C, k: i32) {
        let mut cx = x;
        for ch in s.chars() {
            let g = mini_glyph(ch);
            for (row, line) in g.iter().enumerate() {
                for (col, b) in line.bytes().enumerate() {
                    if b == b'#' {
                        self.fill(cx + col as i32 * k, y + row as i32 * k, k, k, c);
                    }
                }
            }
            cx += 4 * k;
        }
    }

    /// A beveled rectangle: light top/left and dark bottom/right when raised.
    fn bevel(&mut self, x: i32, y: i32, w: i32, h: i32, raised: bool, face: C) {
        if self.t.flat {
            // HUD style: translucent dark panel with a thin chamfered outline;
            // pressed = accent outline and a brighter fill.
            self.fill_bg(x, y, w, h, false);
            self.darken(x, y, w, h, if raised { 0.42 } else { 0.7 });
            let line = if raised { self.t.light } else { self.t.accent };
            self.chamfer_outline(x, y, w, h, line);
            if !raised {
                let acc = self.t.accent;
                self.hline(x + 1, y + 1, w - 2, acc);
            }
            return;
        }
        self.fill(x, y, w, h, face);
        let (tl, br) = if raised {
            (self.t.lighter, self.t.dark)
        } else {
            (self.t.dark, self.t.lighter)
        };
        self.hline(x, y, w, tl);
        self.vline(x, y, h, tl);
        self.hline(x, y + h - 1, w, br);
        self.vline(x + w - 1, y, h, br);
    }

    /// Sunken LCD panel (or, for flat themes, a translucent scanner display).
    fn panel(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if self.t.flat {
            self.fill_bg(x, y, w, h, false);
            self.darken(x, y, w, h, 0.18);
            let acc = self.t.accent;
            let l = 4.min(w / 3).min(h / 2);
            for (bx, by, dx, dy) in [
                (x - 1, y - 1, 1, 1),
                (x + w, y - 1, -1, 1),
                (x - 1, y + h, 1, -1),
                (x + w, y + h, -1, -1),
            ] {
                for i in 0..l {
                    self.px(bx + dx * i, by, acc);
                    self.px(bx, by + dy * i, acc);
                }
            }
            return;
        }
        self.fill(x, y, w, h, self.t.lcd);
        self.hline(x - 1, y - 1, w + 2, self.t.dark);
        self.vline(x - 1, y - 1, h + 2, self.t.dark);
        self.hline(x - 1, y + h, w + 2, self.t.light);
        self.vline(x + w, y - 1, h + 2, self.t.light);
    }

    fn groove(&mut self, x: i32, y: i32, w: i32) {
        self.hline(x, y, w, self.t.dark);
        self.hline(x, y + 1, w, self.t.light);
    }

    fn text5(&mut self, x: i32, y: i32, s: &str, c: C) {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(g) = font::glyph(ch) {
                for (row, line) in g.iter().enumerate() {
                    for (col, b) in line.bytes().enumerate() {
                        if b == b'#' {
                            self.px(cx + col as i32, y + row as i32, c);
                        }
                    }
                }
            }
            cx += 5;
        }
    }

    pub fn text3(&mut self, x: i32, y: i32, s: &str, c: C) {
        // On patterned themes give labels a dark backing so they stay legible.
        if self.t.pattern.is_some() {
            let w = Canvas::text3_width(s);
            let backing = self.t.bg;
            self.fill(x - 1, y - 1, w + 2, 7, backing);
        }
        let mut cx = x;
        for ch in s.chars() {
            let g = mini_glyph(ch);
            for (row, line) in g.iter().enumerate() {
                for (col, b) in line.bytes().enumerate() {
                    if b == b'#' {
                        self.px(cx + col as i32, y + row as i32, c);
                    }
                }
            }
            cx += 4;
        }
    }

    fn text3_width(s: &str) -> i32 {
        (s.chars().count() as i32 * 4 - 1).max(0)
    }

    fn mask(&mut self, x: i32, y: i32, w: usize, mask: &[bool], c: C) {
        for (i, &on) in mask.iter().enumerate() {
            if on {
                self.px(x + (i % w) as i32, y + (i / w) as i32, c);
            }
        }
    }

    /// Decorative title stripes (pairs of light/dark lines).
    fn stripes(&mut self, x: i32, y: i32, w: i32, bright: bool) {
        let (a, b) = if bright {
            (self.t.lighter, self.t.dark)
        } else {
            (self.t.light, self.t.bg2)
        };
        for i in 0..3 {
            self.hline(x, y + 1 + i * 3, w, a);
            self.hline(x, y + 2 + i * 3, w, b);
        }
    }

    fn blit(&mut self, src: &RgbaImage, dx: i32, dy: i32) {
        for y in 0..src.height() {
            for x in 0..src.width() {
                let p = src.get_pixel(x, y);
                self.px(dx + x as i32, dy + y as i32, [p[0], p[1], p[2]]);
            }
        }
    }
}

fn lerp(a: C, b: C, t: f32) -> C {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

fn title_bar(t: &Theme, c: &mut Canvas, x: i32, y: i32, text: &str, active: bool, shade: bool) {
    let emblem_w = if t.emblem.is_some() { 12 } else { 0 };
    let tw = Canvas::text3_width(text) + 10 + emblem_w;
    c.origin = (-x, -y);
    c.fill_bg(x, y, 275, 14, !active);
    c.origin = (0, 0);
    let tx = x + (275 - tw) / 2;
    c.stripes(x + 10, y + 2, tx - x - 14, active);
    c.stripes(tx + tw + 4, y + 2, 275 - (tx - x) - tw - 4 - 38, active);
    if let Some(e) = t.emblem {
        c.fill(tx + 3, y + 1, 11, 11, t.bg);
        e(c, tx + 4, y + 2);
    }
    let color = if active { t.title_color } else { t.label_dim };
    c.text3(tx + 5 + emblem_w, y + 4, text, color);
    // Baked unpressed buttons.
    draw_title_button(t, c, x + 6, y + 3, 'o');
    draw_title_button(t, c, x + 244, y + 3, '_');
    draw_title_button(t, c, x + 254, y + 3, if shade { 'u' } else { 's' });
    draw_title_button(t, c, x + 264, y + 3, 'x');
}

fn draw_title_button(t: &Theme, c: &mut Canvas, x: i32, y: i32, kind: char) {
    draw_title_button_state(t, c, x, y, kind, false);
}

fn draw_title_button_state(t: &Theme, c: &mut Canvas, x: i32, y: i32, kind: char, down: bool) {
    c.bevel(x, y, 9, 9, !down, if down { t.face_down } else { t.face });
    let col = t.label;
    match kind {
        'o' => {
            c.fill(x + 3, y + 3, 3, 3, col);
        }
        '_' => c.hline(x + 2, y + 6, 5, col),
        's' => {
            c.hline(x + 2, y + 2, 5, col);
            c.hline(x + 2, y + 6, 5, col);
        }
        'u' => {
            c.outline(x + 2, y + 2, 5, 5, col);
        }
        'x' => {
            for i in 0..5 {
                c.px(x + 2 + i, y + 2 + i, col);
                c.px(x + 6 - i, y + 2 + i, col);
            }
        }
        _ => {}
    }
}

fn main_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 275, 116, t.bg);
    c.fill(0, 0, 275, 14, t.bg);
    // t.lcd panel: indicator + time + visualiser.
    c.panel(23, 23, 79, 38);
    // Title marquee panel.
    c.panel(108, 23, 160, 16);
    // Bitrate / sample rate panel.
    c.panel(108, 41, 94, 11);
    c.text3(128, 44, "KBPS", t.dim_green);
    c.text3(169, 44, "KHZ", t.dim_green);
    // Mono/stereo slots and slider labels are sprites; add grooves for structure.
    c.groove(10, 66, 255);
    c.groove(10, 84, 255);
    c.text3(107, 52, "VOL", t.label_dim);
    c.text3(177, 52, "BAL", t.label_dim);
    // Side rails.
    c.vline(0, 14, 102, t.light);
    c.vline(274, 14, 102, t.dark);
    c.hline(0, 115, 275, t.dark);
    c.img
}

fn titlebar_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 344, 87, t.bg);
    title_bar(t, &mut c, 27, 0, t.title_main, true, false);
    title_bar(t, &mut c, 27, 15, t.title_main, false, false);
    // Shade mode bars: title text area is drawn by the app; leave a panel.
    for (y, active) in [(29, true), (42, false)] {
        c.fill_bg(27, y, 275, 14, !active);
        c.stripes(27 + 16, y + 2, 60, active);
        c.panel(27 + 79, y + 3, 95, 8);
        c.panel(27 + 199, y + 3, 24, 8);
        draw_title_button(t, &mut c, 27 + 6, y + 3, 'o');
        draw_title_button(t, &mut c, 27 + 244, y + 3, '_');
        draw_title_button(t, &mut c, 27 + 254, y + 3, 'u');
        draw_title_button(t, &mut c, 27 + 264, y + 3, 'x');
    }
    // Button sprites.
    for (i, k) in ['o', '_', 'x'].iter().enumerate() {
        draw_title_button_state(t, &mut c, i as i32 * 9, 0, *k, false);
        draw_title_button_state(t, &mut c, i as i32 * 9, 9, *k, true);
    }
    draw_title_button_state(t, &mut c, 0, 18, 's', false);
    draw_title_button_state(t, &mut c, 9, 18, 's', true);
    draw_title_button_state(t, &mut c, 0, 27, 'u', false);
    draw_title_button_state(t, &mut c, 9, 27, 'u', true);
    // Shade position bar: bg 17x7 at (0,36), thumbs 3x7 at 17,20,23.
    c.fill(0, 36, 17, 7, t.lcd);
    c.hline(0, 39, 17, t.dark);
    c.fill(17, 36, 3, 7, t.lighter);
    c.fill(20, 36, 3, 7, t.label);
    c.fill(23, 36, 3, 7, t.lighter);
    // Clutter bar at (304,0) 8x43 and disabled at (312,0).
    for (x, col) in [(304, t.label), (312, t.label_dim)] {
        c.fill(x, 0, 8, 43, t.bg2);
        c.vline(x, 0, 43, t.dark);
        c.vline(x + 7, 0, 43, t.light);
        for (i, ch) in ['O', 'A', 'I', 'D', 'V'].iter().enumerate() {
            let yy = [1, 9, 16, 23, 31][i];
            c.text3(x + 3, yy, &ch.to_string(), col);
        }
    }
    // Pressed clutter buttons.
    let pressed = [
        (304, 47, 8, 'O'),
        (312, 55, 7, 'A'),
        (320, 62, 7, 'I'),
        (328, 69, 8, 'D'),
        (336, 77, 7, 'V'),
    ];
    for (x, y, h, ch) in pressed {
        c.fill(x, y, 8, h, t.face_down);
        c.text3(x + 3, y + 1, &ch.to_string(), t.white);
    }
    c.img
}

fn transport_glyph(_t: &Theme, c: &mut Canvas, x: i32, y: i32, kind: &str, col: C) {
    // Draws within a 23x18 (or 22x16 for eject) button; (x,y) is the glyph origin.
    match kind {
        "prev" => {
            c.vline(x, y, 7, col);
            for i in 0..4 {
                c.vline(x + 1 + i, y + 3 - i, 1 + i * 2, col);
                c.vline(x + 5 + i, y + 3 - i, 1 + i * 2, col);
            }
        }
        "play" => {
            for i in 0..4 {
                c.vline(x + i, y + i, 7 - i * 2, col);
            }
        }
        "pause" => {
            c.fill(x, y, 3, 7, col);
            c.fill(x + 5, y, 3, 7, col);
        }
        "stop" => c.fill(x, y, 7, 7, col),
        "next" => {
            for i in 0..4 {
                c.vline(x + i, y + i, 7 - i * 2, col);
                c.vline(x + 4 + i, y + i, 7 - i * 2, col);
            }
            c.vline(x + 8, y, 7, col);
        }
        "eject" => {
            for i in 0..4 {
                c.hline(x + 3 - i, y + i, 1 + i * 2, col);
            }
            c.hline(x, y + 5, 7, col);
        }
        _ => {}
    }
}

fn cbuttons_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 136, 36, t.bg);
    let buttons = [
        ("prev", 0, 23, 8),
        ("play", 23, 23, 10),
        ("pause", 46, 23, 8),
        ("stop", 69, 23, 8),
        ("next", 92, 22, 7),
    ];
    for (kind, x, w, gx) in buttons {
        for (down, y) in [(false, 0), (true, 18)] {
            c.bevel(x, y, w, 18, !down, if down { t.face_down } else { t.face });
            let off = if down { 1 } else { 0 };
            transport_glyph(t, &mut c, x + gx + off, y + 5 + off, kind, t.label);
        }
    }
    for (down, y) in [(false, 0), (true, 16)] {
        c.bevel(
            114,
            y,
            22,
            16,
            !down,
            if down { t.face_down } else { t.face },
        );
        let off = if down { 1 } else { 0 };
        transport_glyph(t, &mut c, 114 + 8 + off, y + 5 + off, "eject", t.label);
    }
    c.img
}

#[allow(clippy::too_many_arguments)]
fn toggle_button(
    t: &Theme,
    c: &mut Canvas,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    label: &str,
    on: bool,
    down: bool,
    mini: bool,
) {
    c.bevel(x, y, w, h, !down, if down { t.face_down } else { t.face });
    let col = if on { t.green } else { t.label_dim };
    let off = if down { 1 } else { 0 };
    if mini {
        let tw = Canvas::text3_width(label);
        c.text3(x + (w - tw) / 2 + off, y + (h - 5) / 2 + off, label, col);
    } else {
        let tw = label.chars().count() as i32 * 5 - 1;
        c.text5(x + (w - tw) / 2 + off, y + (h - 5) / 2 + off, label, col);
    }
    if on {
        // lit indicator dot
        c.px(x + 2, y + 2, t.green);
    }
}

fn shufrep_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 92, 85, t.bg);
    for (i, (on, down)) in [(false, false), (false, true), (true, false), (true, true)]
        .iter()
        .enumerate()
    {
        let y = i as i32 * 15;
        toggle_button(t, &mut c, 0, y, 28, 15, "REPEAT", *on, *down, true);
        toggle_button(t, &mut c, 28, y, 47, 15, "SHUFFLE", *on, *down, true);
    }
    // EQ / PL buttons: off (0,61) (23,61); off-down (46,61) (69,61); on (0,73) (23,73); on-down (46,73) (69,73)
    for (on, y) in [(false, 61), (true, 73)] {
        for (down, x0) in [(false, 0), (true, 46)] {
            toggle_button(t, &mut c, x0, y, 23, 12, "EQ", on, down, true);
            toggle_button(t, &mut c, x0 + 23, y, 23, 12, "PL", on, down, true);
        }
    }
    c.img
}

fn posbar_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 307, 10, t.bg);
    c.fill(0, 0, 248, 10, t.bg);
    c.fill(1, 3, 246, 4, t.dark);
    c.hline(1, 3, 246, t.black);
    c.hline(1, 7, 246, t.light);
    for (x, down) in [(248, false), (278, true)] {
        c.bevel(x, 0, 29, 10, !down, if down { t.face_down } else { t.face });
        for i in 0..3 {
            c.vline(x + 11 + i * 3, 2, 6, if down { t.dark } else { t.lighter });
        }
    }
    c.img
}

fn slider_thumb(t: &Theme, c: &mut Canvas, x: i32, y: i32, down: bool) {
    c.bevel(x, y, 14, 11, !down, if down { t.face_down } else { t.face });
    c.vline(x + 6, y + 2, 7, t.dark);
    c.vline(x + 7, y + 2, 7, t.lighter);
}

fn volume_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 68, 433, t.bg);
    for f in 0..28 {
        let y = f * 15;
        c.fill(0, y, 68, 13, t.bg);
        c.fill(1, y + 4, 66, 5, t.dark);
        c.hline(1, y + 4, 66, t.black);
        let tt = f as f32 / 27.0;
        let w = (tt * 64.0) as i32;
        for x in 0..w {
            let tt = x as f32 / 64.0;
            let col = if tt < 0.5 {
                lerp(t.grad_lo, t.grad_mid, tt * 2.0)
            } else {
                lerp(t.grad_mid, t.grad_hi, (tt - 0.5) * 2.0)
            };
            c.vline(2 + x, y + 5, 3, col);
        }
    }
    slider_thumb(t, &mut c, 0, 422, true);
    slider_thumb(t, &mut c, 15, 422, false);
    c.img
}

fn balance_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 47, 433, t.bg);
    for f in 0..28 {
        let y = f * 15;
        c.fill(9, y, 38, 13, t.bg);
        c.fill(10, y + 4, 36, 5, t.dark);
        c.hline(10, y + 4, 36, t.black);
        // Frame = amount of deviation from centre (used for both directions).
        let center = 28;
        let w = (f as f32 / 27.0 * 17.0) as i32;
        for i in 0..w {
            let col = lerp(t.grad_lo, t.grad_hi, i as f32 / 17.0);
            c.vline(center - 1 - i, y + 5, 3, col);
            c.vline(center + i, y + 5, 3, col);
        }
        c.vline(center - 1, y + 5, 3, t.label_dim);
    }
    slider_thumb(t, &mut c, 0, 422, true);
    slider_thumb(t, &mut c, 15, 422, false);
    c.img
}

fn monoster_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 56, 24, t.bg);
    for (y, on) in [(0, true), (12, false)] {
        let col = if on { t.green } else { t.dim_green };
        c.panel(1, y + 1, 27, 10);
        c.text3(3, y + 3, "STEREO", col);
        c.panel(30, y + 1, 25, 10);
        c.text3(34, y + 3, "MONO", col);
    }
    c.img
}

fn playpaus_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 42, 9, t.lcd);
    // play
    for i in 0..4 {
        c.vline(2 + i, 1 + i, 7 - i * 2, t.green);
    }
    // pause
    c.fill(9 + 1, 1, 3, 7, t.green);
    c.fill(9 + 5, 1, 3, 7, t.green);
    // stop
    c.fill(18 + 1, 1, 7, 7, t.green);
    // (27,0) blank; work indicator off (36,0) / on (39,0)
    c.fill(36, 0, 3, 9, t.lcd);
    c.vline(37, 1, 7, t.dim_green);
    c.fill(39, 0, 3, 9, t.lcd);
    c.vline(40, 1, 7, t.green);
    c.img
}

fn numbers_bmp(t: &Theme, ex: bool) -> RgbaImage {
    let w = if ex { 108 } else { 99 };
    let mut c = Canvas::new(t, w, 13, t.lcd);
    for d in 0..10 {
        let m = font::digit_mask(d);
        c.mask(d as i32 * 9, 0, 9, &m, t.green);
    }
    if ex {
        let m = font::segment_mask(0b1000000);
        c.mask(99, 0, 9, &m, t.green);
    }
    c.img
}

/// Build nums_ex.bmp from numbers.bmp by appending a minus glyph in the digit color.
pub fn nums_ex_from_numbers(t: &Theme, numbers: &RgbaImage) -> RgbaImage {
    let mut c = Canvas::new(t, 108, 13, t.lcd);
    c.blit(numbers, 0, 0);
    // Copy the blank cell as background for the minus cell.
    for y in 0..13 {
        for x in 0..9 {
            let p = numbers.get_pixel(
                (90 + x).min(numbers.width() - 1),
                y.min(numbers.height() - 1),
            );
            c.px(99 + x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    // Find the digit color: brightest pixel of the "8" glyph.
    let mut best = [0u8, 255, 0];
    let mut best_l = 0u32;
    for y in 0..13.min(numbers.height()) {
        for x in 72..81.min(numbers.width()) {
            let p = numbers.get_pixel(x, y);
            let l = p[0] as u32 + p[1] as u32 + p[2] as u32;
            if l > best_l {
                best_l = l;
                best = [p[0], p[1], p[2]];
            }
        }
    }
    c.hline(101, 6, 5, best);
    c.img
}

fn text_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 155, 18, t.lcd);
    let rows: [&[char]; 3] = [
        &[
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q',
            'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '"', '@', ' ', ' ', ' ',
        ],
        &[
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '…', '.', ':', '(', ')', '-', '\'',
            '!', '_', '+', '\\', '/', '[', ']', '^', '&', '%', ',', '=', '$', '#',
        ],
        &[
            'Å', 'Ö', 'Ä', '?', '*', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
            ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
        ],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (col, ch) in row.iter().enumerate() {
            c.text5(col as i32 * 5, r as i32 * 6, &ch.to_string(), t.green);
        }
    }
    c.img
}

fn eqmain_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 275, 315, t.bg);
    // Background 0..116
    c.fill_bg(0, 0, 275, 116, false);
    c.vline(0, 14, 102, t.light);
    c.vline(274, 14, 102, t.dark);
    c.hline(0, 115, 275, t.dark);
    c.panel(86, 17, 113, 19);
    // Slider well labels
    c.text3(14, 104, "PREAMP", t.label_dim);
    c.text3(49, 38, "+12DB", t.label_dim);
    c.text3(55, 64, "0DB", t.label_dim);
    c.text3(49, 92, "-12DB", t.label_dim);
    let labels = [
        "60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K",
    ];
    for (i, l) in labels.iter().enumerate() {
        let x = 78 + i as i32 * 18;
        let tw = Canvas::text3_width(l);
        c.text3(x + (14 - tw) / 2, 104, l, t.label_dim);
    }
    // Close button sprites
    draw_title_button_state(t, &mut c, 0, 116, 'x', false);
    draw_title_button_state(t, &mut c, 0, 125, 'x', true);
    // ON / AUTO buttons
    let on_btn = [
        (10, false, false),
        (128, false, true),
        (69, true, false),
        (187, true, true),
    ];
    for (x, on, down) in on_btn {
        toggle_button(t, &mut c, x, 119, 26, 12, "ON", on, down, true);
    }
    let auto_btn = [
        (35, false, false),
        (153, false, true),
        (94, true, false),
        (212, true, true),
    ];
    for (x, on, down) in auto_btn {
        toggle_button(t, &mut c, x, 119, 32, 12, "AUTO", on, down, true);
    }
    // Title bars
    title_bar(t, &mut c, 0, 134, t.title_eq, true, false);
    title_bar(t, &mut c, 0, 149, t.title_eq, false, false);
    // Slider backgrounds: 28 frames, 14x63 at (13 + i*15, 164 + row*65)
    for f in 0..28 {
        let x = 13 + (f % 14) * 15;
        let y = 164 + (f / 14) * 65;
        c.fill(x, y, 14, 63, t.bg);
        c.fill(x + 5, y + 2, 4, 59, t.dark);
        c.vline(x + 5, y + 2, 59, t.black);
        // Lit bar from center to level: frame 0 = -12 dB (bottom), 27 = +12 dB (top)
        let tt = f as f32 / 27.0;
        let center = y + 31;
        let level = y + 5 + ((1.0 - tt) * 51.0) as i32 + 5;
        let col = if tt < 0.5 {
            lerp(t.grad_hi, t.grad_mid, tt * 2.0)
        } else {
            lerp(t.grad_mid, t.grad_lo, (tt - 0.5) * 2.0)
        };
        let (a, b) = if level < center {
            (level, center)
        } else {
            (center, level)
        };
        c.fill(x + 6, a, 2, (b - a).max(1), col);
        c.hline(x + 3, center, 8, t.label_dim);
    }
    // Thumb sprites 11x11 at (0,164) and (0,176)
    for (y, down) in [(164, false), (176, true)] {
        c.bevel(0, y, 11, 11, !down, if down { t.face_down } else { t.face });
        c.hline(2, y + 5, 7, if down { t.dark } else { t.lighter });
    }
    // Presets button
    for (y, down) in [(164, false), (176, true)] {
        c.bevel(
            224,
            y,
            44,
            12,
            !down,
            if down { t.face_down } else { t.face },
        );
        let off = if down { 1 } else { 0 };
        c.text3(224 + 9 + off, y + 4 + off, "PRESETS", t.label);
    }
    // Graph background (0,294) 113x19 and color column (115,294)
    c.fill(0, 294, 113, 19, t.lcd);
    c.hline(0, 294 + 9, 113, [40, 60, 40]);
    for i in 0..10 {
        c.vline(i * 12 + 6, 294, 19, [20, 30, 20]);
    }
    for y in 0..19 {
        let tt = y as f32 / 18.0;
        let col = if tt < 0.5 {
            lerp(t.grad_lo, t.grad_mid, tt * 2.0)
        } else {
            lerp(t.grad_mid, t.grad_hi, (tt - 0.5) * 2.0)
        };
        c.px(115, 294 + y, col);
    }
    c.img
}

fn eq_ex_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 275, 57, t.bg);
    for (y, active) in [(0, true), (15, false)] {
        c.fill_bg(0, y, 275, 14, !active);
        c.stripes(16, y + 2, 60, active);
        c.text3(
            90,
            y + 4,
            t.title_eq,
            if active { t.title_color } else { t.label_dim },
        );
        c.stripes(165, y + 2, 75, active);
        draw_title_button(t, &mut c, 254, y + 3, 'u');
        draw_title_button(t, &mut c, 264, y + 3, 'x');
    }
    draw_title_button_state(t, &mut c, 1, 38, 'u', false);
    draw_title_button_state(t, &mut c, 1, 47, 'u', true);
    draw_title_button_state(t, &mut c, 11, 38, 'x', false);
    draw_title_button_state(t, &mut c, 11, 47, 'x', true);
    c.img
}

/// Compose an eq_ex.bmp for skins that lack one using the eqmain title bar.
pub fn eq_ex_from_eqmain(t: &Theme, eqmain: &RgbaImage) -> RgbaImage {
    let mut c = Canvas::new(t, 275, 57, t.bg);
    for (dy, sy) in [(0u32, 134u32), (15, 149)] {
        for y in 0..14 {
            for x in 0..275u32 {
                if x < eqmain.width() && sy + y < eqmain.height() {
                    let p = eqmain.get_pixel(x, sy + y);
                    c.px(x as i32, (dy + y) as i32, [p[0], p[1], p[2]]);
                }
            }
        }
    }
    draw_title_button_state(t, &mut c, 1, 38, 'u', false);
    draw_title_button_state(t, &mut c, 1, 47, 'u', true);
    draw_title_button_state(t, &mut c, 11, 38, 'x', false);
    draw_title_button_state(t, &mut c, 11, 47, 'x', true);
    c.img
}

fn pl_button(t: &Theme, c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, label: &str) {
    c.bevel(x, y, w, h, true, t.face);
    let tw = Canvas::text3_width(label);
    c.text3(x + (w - tw) / 2, y + (h - 5) / 2, label, t.label);
}

fn pledit_bmp(t: &Theme) -> RgbaImage {
    let mut c = Canvas::new(t, 280, 186, t.bg);
    // Title row (active y=0, inactive y=21)
    for (y, active) in [(0, true), (21, false)] {
        let fg = if active { t.label } else { t.label_dim };
        // TL 25x20
        c.fill_bg(0, y, 25, 20, !active);
        c.vline(0, y, 20, t.light);
        c.stripes(4, y + 5, 21, active);
        // Title 100x20
        c.fill_bg(26, y, 100, 20, !active);
        c.stripes(26, y + 5, 12, active);
        let ptw = Canvas::text3_width(t.title_pl);
        let px0 = 26 + (100 - ptw) / 2;
        c.text3(
            px0,
            y + 7,
            t.title_pl,
            if active { t.title_color } else { fg },
        );
        c.stripes(26 + 86, y + 5, 14, active);
        // Top tile 25x20
        c.fill_bg(127, y, 25, 20, !active);
        c.stripes(127, y + 5, 25, active);
        // TR 25x20 with shade + close buttons
        c.fill_bg(153, y, 25, 20, !active);
        c.vline(177, y, 20, t.dark);
        draw_title_button(t, &mut c, 153 + 5, y + 3, 's');
        draw_title_button(t, &mut c, 153 + 15, y + 3, 'x');
    }
    // Left tile (0,42) 12x29 ; right tile (31,42) 20x29 with scroll track
    c.fill_bg(0, 42, 12, 29, false);
    c.vline(0, 42, 29, t.light);
    c.vline(11, 42, 29, t.dark);
    c.fill_bg(31, 42, 20, 29, false);
    c.vline(50, 42, 29, t.dark);
    c.fill(31 + 5, 42, 8, 29, t.dark);
    c.vline(31 + 5, 42, 29, t.black);
    c.vline(31 + 12, 42, 29, t.light);
    // Pressed close (52,42) and shade (62,42)
    draw_title_button_state(t, &mut c, 52, 42, 'x', true);
    draw_title_button_state(t, &mut c, 62, 42, 's', true);
    // Scroll thumbs 8x18 at (52,53) and (61,53)
    for (x, down) in [(52, false), (61, true)] {
        c.bevel(x, 53, 8, 18, !down, if down { t.face_down } else { t.face });
        for i in 0..3 {
            c.hline(
                x + 2,
                53 + 6 + i * 3,
                4,
                if down { t.dark } else { t.lighter },
            );
        }
    }
    // Shade mode backgrounds: left (72,42) 25x14, tile (72,57) 25x14,
    // right active (99,42) 50x14, right inactive (99,57) 50x14
    c.fill_bg(72, 42, 25, 14, false);
    c.vline(72, 42, 14, t.light);
    c.stripes(76, 44, 21, true);
    c.fill_bg(72, 57, 25, 14, false);
    c.stripes(72, 59, 25, true);
    for (y, active) in [(42, true), (57, false)] {
        c.fill_bg(99, y, 50, 14, !active);
        c.stripes(99, y + 2, 25, active);
        draw_title_button(t, &mut c, 99 + 30, y + 3, 'u');
        draw_title_button(t, &mut c, 99 + 40, y + 3, 'x');
    }
    draw_title_button_state(t, &mut c, 150, 42, 'u', true);
    // Bottom tile (179,0) 25x38
    c.fill_bg(179, 0, 25, 38, false);
    c.hline(179, 37, 25, t.dark);
    c.groove(179, 2, 25);
    // Bottom-left corner (0,72) 125x38 with ADD/REM/SEL/MISC buttons
    c.fill_bg(0, 72, 125, 38, false);
    c.vline(0, 72, 38, t.light);
    c.hline(0, 109, 125, t.dark);
    c.groove(1, 74, 124);
    pl_button(t, &mut c, 11, 92, 25, 18, "ADD");
    pl_button(t, &mut c, 40, 92, 25, 18, "REM");
    pl_button(t, &mut c, 70, 92, 25, 18, "SEL");
    pl_button(t, &mut c, 99, 92, 25, 18, "MISC");
    // Bottom-right corner (126,72) 150x38: time panel + LIST button
    c.fill_bg(126, 72, 150, 38, false);
    c.vline(275, 72, 38, t.dark);
    c.hline(126, 109, 150, t.dark);
    c.groove(126, 74, 150);
    c.panel(126 + 10, 72 + 22, 86, 11);
    pl_button(t, &mut c, 126 + 104, 92, 25, 18, "LIST");
    // Mini transport glyphs row (decorative) at y=79
    let glyphs = ["prev", "play", "pause", "stop", "next"];
    for (i, g) in glyphs.iter().enumerate() {
        transport_glyph(t, &mut c, 126 + 14 + i as i32 * 12, 72 + 8, g, t.label_dim);
    }
    c.img
}

pub fn generate() -> Skin {
    generate_theme(&BASE_THEME)
}

pub fn generate_theme(t: &Theme) -> Skin {
    let mut images = HashMap::new();
    images.insert(SkinImage::Main, main_bmp(t));
    images.insert(SkinImage::Titlebar, titlebar_bmp(t));
    images.insert(SkinImage::CButtons, cbuttons_bmp(t));
    images.insert(SkinImage::ShufRep, shufrep_bmp(t));
    images.insert(SkinImage::PosBar, posbar_bmp(t));
    images.insert(SkinImage::Volume, volume_bmp(t));
    images.insert(SkinImage::Balance, balance_bmp(t));
    images.insert(SkinImage::MonoSter, monoster_bmp(t));
    images.insert(SkinImage::PlayPaus, playpaus_bmp(t));
    images.insert(SkinImage::Numbers, numbers_bmp(t, false));
    images.insert(SkinImage::NumsEx, numbers_bmp(t, true));
    images.insert(SkinImage::Text, text_bmp(t));
    images.insert(SkinImage::EqMain, eqmain_bmp(t));
    images.insert(SkinImage::EqEx, eq_ex_bmp(t));
    images.insert(SkinImage::PlEdit, pledit_bmp(t));
    let _ = t.accent;
    if let Some(deco) = t.decorate {
        for (which, img) in images.iter_mut() {
            let mut c = Canvas {
                img: std::mem::take(img),
                t: *t,
                origin: (0, 0),
            };
            deco(&mut c, *which);
            *img = c.img;
        }
    }
    Skin {
        name: t.name.to_string(),
        images,
        pledit: PleditColors::parse(t.pledit_txt),
        viscolors: VisColors::parse(t.viscolor_txt),
        provided: SkinImage::ALL.to_vec(),
    }
}
