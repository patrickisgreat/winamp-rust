//! Shared skinned-widget interaction helpers (buttons, sliders, title bars).

use winamp_skin::sprites::Sprite;

use crate::app::Hot;
use crate::gfx::{Gfx, Mouse};
use crate::windows::Win;

/// Draw a two-state button and report a click (press + release inside).
pub fn button(g: &Gfx, m: &Mouse, hot: &mut Option<Hot>, win: Win, id: u16, x: u32, y: u32, normal: Sprite, down: Sprite) -> bool {
    let over = m.over(x, y, normal.w, normal.h);
    let me = Hot::Button(win, id);
    if m.pressed && over && hot.is_none() {
        *hot = Some(me);
    }
    let is_hot = *hot == Some(me);
    let visual_down = is_hot && over && m.down;
    g.sprite(if visual_down { down } else { normal }, x as f32, y as f32);
    is_hot && m.released && over
}

/// Invisible hit area behaving like a button (for baked-in title bar buttons).
pub fn hit_button(m: &Mouse, hot: &mut Option<Hot>, win: Win, id: u16, x: u32, y: u32, w: u32, h: u32) -> (bool, bool) {
    let over = m.over(x, y, w, h);
    let me = Hot::Button(win, id);
    if m.pressed && over && hot.is_none() {
        *hot = Some(me);
    }
    let is_hot = *hot == Some(me);
    (is_hot && m.released && over, is_hot && over && m.down)
}

/// Horizontal slider logic. `value` is 0..=1. Returns the (possibly updated)
/// value, whether it is being dragged, and whether the drag just ended.
pub struct SliderResult {
    pub value: f32,
    pub dragging: bool,
    pub released: bool,
}

pub fn hslider(m: &Mouse, hot: &mut Option<Hot>, win: Win, id: u16, x: u32, y: u32, w: u32, h: u32, thumb_w: u32, value: f32) -> SliderResult {
    let me = Hot::Slider(win, id);
    let over = m.over(x, y, w, h);
    let mut value = value;
    if m.pressed && over && hot.is_none() {
        *hot = Some(me);
    }
    let dragging = *hot == Some(me) && (m.down || m.released);
    if dragging {
        if let Some(p) = m.pos {
            let travel = (w - thumb_w) as f32;
            value = ((p.x - x as f32 - thumb_w as f32 / 2.0) / travel).clamp(0.0, 1.0);
        }
    }
    SliderResult { value, dragging, released: *hot == Some(me) && m.released }
}

/// Vertical slider: value 0 at the bottom, 1 at the top.
pub fn vslider(m: &Mouse, hot: &mut Option<Hot>, win: Win, id: u16, x: u32, y: u32, w: u32, h: u32, thumb_h: u32, value: f32) -> SliderResult {
    let me = Hot::Slider(win, id);
    let over = m.over(x, y, w, h);
    let mut value = value;
    if m.pressed && over && hot.is_none() {
        *hot = Some(me);
    }
    let dragging = *hot == Some(me) && (m.down || m.released);
    if dragging {
        if let Some(p) = m.pos {
            let travel = (h - thumb_h) as f32;
            value = (1.0 - (p.y - y as f32 - thumb_h as f32 / 2.0) / travel).clamp(0.0, 1.0);
        }
    }
    SliderResult { value, dragging, released: *hot == Some(me) && m.released }
}
