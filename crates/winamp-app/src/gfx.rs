//! Drawing helpers: sprite blitting through egui's painter at an integer
//! scale, bitmap font text, and small pixel surfaces for the visualiser.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Painter, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use winamp_skin::sprites::{self, Sprite};
use winamp_skin::{Rgb, Skin, SkinImage};

pub struct Textures {
    map: HashMap<SkinImage, TextureHandle>,
}

impl Textures {
    pub fn from_skin(ctx: &egui::Context, skin: &Skin) -> Self {
        let mut map = HashMap::new();
        for which in SkinImage::ALL {
            let img = skin.image(which);
            let ci = ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            let tex = ctx.load_texture(format!("skin-{}", which.file_stem()), ci, TextureOptions::NEAREST);
            map.insert(which, tex);
        }
        Self { map }
    }

    pub fn get(&self, which: SkinImage) -> &TextureHandle {
        &self.map[&which]
    }
}

pub fn rgb(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A per-window drawing context in skin pixel units.
pub struct Gfx<'a> {
    pub painter: &'a Painter,
    pub textures: &'a Textures,
    pub skin: &'a Skin,
    pub origin: Pos2,
    pub scale: f32,
}

impl<'a> Gfx<'a> {
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(
            self.origin + Vec2::new(x * self.scale, y * self.scale),
            Vec2::new(w * self.scale, h * self.scale),
        )
    }

    /// Blit a sprite at (x, y) in skin pixels.
    pub fn sprite(&self, sp: Sprite, x: f32, y: f32) {
        self.sprite_sized(sp, x, y, sp.w as f32, sp.h as f32);
    }

    /// Blit a sprite stretched to w x h.
    pub fn sprite_sized(&self, sp: Sprite, x: f32, y: f32, w: f32, h: f32) {
        let tex = self.textures.get(sp.img);
        let size = tex.size_vec2();
        let uv = Rect::from_min_max(
            Pos2::new(sp.x as f32 / size.x, sp.y as f32 / size.y),
            Pos2::new((sp.x + sp.w) as f32 / size.x, (sp.y + sp.h) as f32 / size.y),
        );
        self.painter.image(tex.id(), self.rect(x, y, w, h), uv, Color32::WHITE);
    }

    /// Blit a horizontal slice of a sprite (used for clipping the marquee).
    pub fn sprite_clipped(&self, sp: Sprite, x: f32, y: f32, clip: Rect) {
        let r = self.rect(x, y, sp.w as f32, sp.h as f32);
        let visible = r.intersect(clip);
        if visible.is_negative() || visible.width() <= 0.0 {
            return;
        }
        let tex = self.textures.get(sp.img);
        let size = tex.size_vec2();
        let u0 = (sp.x as f32 + (visible.min.x - r.min.x) / self.scale) / size.x;
        let u1 = (sp.x as f32 + (visible.max.x - r.min.x) / self.scale) / size.x;
        let v0 = sp.y as f32 / size.y;
        let v1 = (sp.y + sp.h) as f32 / size.y;
        self.painter.image(tex.id(), visible, Rect::from_min_max(Pos2::new(u0, v0), Pos2::new(u1, v1)), Color32::WHITE);
    }

    /// Tile a sprite horizontally across `w` pixels.
    pub fn tile_h(&self, sp: Sprite, x: f32, y: f32, w: f32) {
        let mut cx = 0.0;
        let clip = self.rect(x, y, w, sp.h as f32);
        while cx < w {
            self.sprite_clipped(sp, x + cx, y, clip);
            cx += sp.w as f32;
        }
    }

    /// Tile a sprite vertically across `h` pixels.
    pub fn tile_v(&self, sp: Sprite, x: f32, y: f32, h: f32) {
        let mut cy = 0.0;
        while cy < h {
            let remaining = (h - cy).min(sp.h as f32);
            let part = Sprite { h: remaining as u32, ..sp };
            self.sprite(part, x, y + cy);
            cy += sp.h as f32;
        }
    }

    pub fn fill(&self, x: f32, y: f32, w: f32, h: f32, c: Color32) {
        self.painter.rect_filled(self.rect(x, y, w, h), 0.0, c);
    }

    /// Draw text with the skin's bitmap font. Returns width in skin pixels.
    pub fn text(&self, x: f32, y: f32, s: &str) -> f32 {
        let mut cx = x;
        for ch in s.chars() {
            let (row, col) = sprites::font_lookup(ch).unwrap_or((0, 30));
            self.sprite(sprites::font_cell(col, row), cx, y);
            cx += 5.0;
        }
        cx - x
    }

    /// Draw bitmap text clipped to a rectangle (skin pixel units).
    pub fn text_clipped(&self, x: f32, y: f32, s: &str, clip_x: f32, clip_w: f32) {
        let clip = self.rect(clip_x, y, clip_w, 6.0);
        let mut cx = x;
        for ch in s.chars() {
            if cx + 5.0 > clip_x && cx < clip_x + clip_w {
                let (row, col) = sprites::font_lookup(ch).unwrap_or((0, 30));
                self.sprite_clipped(sprites::font_cell(col, row), cx, y, clip);
            }
            cx += 5.0;
        }
    }

    pub fn digit(&self, x: f32, y: f32, d: u32) {
        let img = if self.skin.has(SkinImage::NumsEx) { SkinImage::NumsEx } else { SkinImage::Numbers };
        let img = if self.skin.has(SkinImage::Numbers) && !self.skin.has(SkinImage::NumsEx) { SkinImage::Numbers } else { img };
        self.sprite(sprites::digit(img, d), x, y);
    }

    pub fn digit_blank(&self, x: f32, y: f32) {
        self.sprite(sprites::DIGIT_BLANK_EX, x, y);
    }

    pub fn minus(&self, x: f32, y: f32, show: bool) {
        if show {
            self.sprite(sprites::DIGIT_MINUS_EX, x, y);
        } else {
            self.sprite(sprites::DIGIT_BLANK_EX, x, y);
        }
    }

    pub fn egui_text(&self, x: f32, y: f32, s: &str, size: f32, color: Color32) -> Rect {
        let pos = self.origin + Vec2::new(x * self.scale, y * self.scale);
        self.painter.text(pos, egui::Align2::LEFT_TOP, s, egui::FontId::proportional(size * self.scale), color)
    }

    pub fn egui_text_right(&self, right_x: f32, y: f32, s: &str, size: f32, color: Color32) -> Rect {
        let pos = self.origin + Vec2::new(right_x * self.scale, y * self.scale);
        self.painter.text(pos, egui::Align2::RIGHT_TOP, s, egui::FontId::proportional(size * self.scale), color)
    }
}

/// A small RGBA surface uploaded as a texture each frame (visualiser, EQ graph).
pub struct PixelSurface {
    pub w: usize,
    pub h: usize,
    img: ColorImage,
    tex: Option<TextureHandle>,
    name: String,
}

impl PixelSurface {
    pub fn new(name: &str, w: usize, h: usize) -> Self {
        Self { w, h, img: ColorImage::new([w, h], Color32::BLACK), tex: None, name: name.to_string() }
    }

    pub fn clear(&mut self, c: Color32) {
        for p in self.img.pixels.iter_mut() {
            *p = c;
        }
    }

    #[inline]
    pub fn px(&mut self, x: i32, y: i32, c: Color32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.img.pixels[y as usize * self.w + x as usize] = c;
        }
    }

    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Color32) {
        let (a, b) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        for y in a..=b {
            self.px(x, y, c);
        }
    }

    pub fn draw(&mut self, ctx: &egui::Context, g: &Gfx, x: f32, y: f32) {
        let img = self.img.clone();
        match &mut self.tex {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => self.tex = Some(ctx.load_texture(self.name.clone(), img, TextureOptions::NEAREST)),
        }
        let tex = self.tex.as_ref().unwrap();
        g.painter.image(
            tex.id(),
            g.rect(x, y, self.w as f32, self.h as f32),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

/// Pointer state for one window, in skin pixel coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mouse {
    pub pos: Option<Pos2>,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub right_pressed: bool,
    pub double_clicked: bool,
    pub scroll: f32,
    pub shift: bool,
    pub ctrl: bool,
}

impl Mouse {
    pub fn read(ctx: &egui::Context, scale: f32) -> Self {
        ctx.input(|i| Mouse {
            pos: i.pointer.latest_pos().map(|p| Pos2::new(p.x / scale, p.y / scale)),
            down: i.pointer.primary_down(),
            pressed: i.pointer.primary_pressed(),
            released: i.pointer.primary_released(),
            right_pressed: i.pointer.secondary_pressed(),
            double_clicked: i.pointer.button_double_clicked(egui::PointerButton::Primary) || i.pointer.button_triple_clicked(egui::PointerButton::Primary),
            scroll: i.raw_scroll_delta.y,
            shift: i.modifiers.shift,
            ctrl: i.modifiers.command,
        })
    }

    pub fn over(&self, x: u32, y: u32, w: u32, h: u32) -> bool {
        match self.pos {
            Some(p) => p.x >= x as f32 && p.x < (x + w) as f32 && p.y >= y as f32 && p.y < (y + h) as f32,
            None => false,
        }
    }

    pub fn over_f(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        match self.pos {
            Some(p) => p.x >= x && p.x < x + w && p.y >= y && p.y < y + h,
            None => false,
        }
    }
}
