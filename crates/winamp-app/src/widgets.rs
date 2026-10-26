//! Shared skinned-widget interaction helpers (buttons, sliders, window dragging).

use egui::Pos2;
use winamp_skin::sprites::Sprite;

use crate::app::{App, Hot};
use crate::gfx::{Gfx, Mouse};
use crate::windows::Win;

/// A rectangle in skin pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl HitBox {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// A box whose top-left corner is one of the sprite `POS_*` tuples.
    pub const fn at(pos: (u32, u32), w: u32, h: u32) -> Self {
        Self {
            x: pos.0,
            y: pos.1,
            w,
            h,
        }
    }
}

/// Outcome of an invisible hit area for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    /// The button was pressed and released inside its area this frame.
    pub clicked: bool,
    /// The pointer is holding the button down right now.
    pub held: bool,
}

/// Horizontal or vertical slider state for one frame. `value` is 0..=1.
pub struct SliderResult {
    pub value: f32,
    pub dragging: bool,
    /// The drag ended this frame; `value` is final.
    pub released: bool,
}

/// Everything a window needs to run its skinned controls for one frame.
pub struct Widgets<'a> {
    pub gfx: &'a Gfx<'a>,
    pub mouse: &'a Mouse,
    pub win: Win,
}

impl<'a> Widgets<'a> {
    pub fn new(gfx: &'a Gfx<'a>, mouse: &'a Mouse, win: Win) -> Self {
        Self { gfx, mouse, win }
    }

    /// Claim `hot` for this control if the pointer just pressed inside `area`.
    /// Returns whether this control owns the current interaction.
    fn claim(&self, hot: &mut Option<Hot>, me: Hot, area: HitBox) -> bool {
        if self.mouse.pressed && self.mouse.over(area.x, area.y, area.w, area.h) && hot.is_none() {
            *hot = Some(me);
        }
        *hot == Some(me)
    }

    /// Draw a two-state button at `pos` and report a click (press + release inside).
    pub fn button(
        &self,
        hot: &mut Option<Hot>,
        id: u16,
        pos: (u32, u32),
        normal: Sprite,
        down: Sprite,
    ) -> bool {
        let area = HitBox::at(pos, normal.w, normal.h);
        let state = self.hit_button(hot, id, area);
        self.gfx.sprite(
            if state.held { down } else { normal },
            area.x as f32,
            area.y as f32,
        );
        state.clicked
    }

    /// Invisible hit area behaving like a button (for baked-in title bar buttons).
    pub fn hit_button(&self, hot: &mut Option<Hot>, id: u16, area: HitBox) -> ButtonState {
        let is_hot = self.claim(hot, Hot::Button(self.win, id), area);
        let over = self.mouse.over(area.x, area.y, area.w, area.h);
        ButtonState {
            clicked: is_hot && self.mouse.released && over,
            held: is_hot && over && self.mouse.down,
        }
    }

    /// Horizontal slider: value 0 at the left, 1 at the right.
    pub fn hslider(
        &self,
        hot: &mut Option<Hot>,
        id: u16,
        area: HitBox,
        thumb_w: u32,
        value: f32,
    ) -> SliderResult {
        self.slider(hot, id, area, value, |pointer| {
            let travel = (area.w - thumb_w) as f32;
            (pointer.x - area.x as f32 - thumb_w as f32 / 2.0) / travel
        })
    }

    /// Vertical slider: value 0 at the bottom, 1 at the top.
    pub fn vslider(
        &self,
        hot: &mut Option<Hot>,
        id: u16,
        area: HitBox,
        thumb_h: u32,
        value: f32,
    ) -> SliderResult {
        self.slider(hot, id, area, value, |pointer| {
            let travel = (area.h - thumb_h) as f32;
            1.0 - (pointer.y - area.y as f32 - thumb_h as f32 / 2.0) / travel
        })
    }

    fn slider(
        &self,
        hot: &mut Option<Hot>,
        id: u16,
        area: HitBox,
        value: f32,
        value_at: impl Fn(Pos2) -> f32,
    ) -> SliderResult {
        let is_hot = self.claim(hot, Hot::Slider(self.win, id), area);
        let dragging = is_hot && (self.mouse.down || self.mouse.released);
        let value = match self.mouse.pos {
            Some(pointer) if dragging => value_at(pointer).clamp(0.0, 1.0),
            _ => value,
        };
        SliderResult {
            value,
            dragging,
            released: is_hot && self.mouse.released,
        }
    }
}

/// Let the user drag the window by any non-control area, and release this
/// window's interaction once the button is up. `may_start` is false over
/// regions (like the playlist rows) that handle their own dragging.
pub fn drag_window(app: &mut App, ctx: &egui::Context, mouse: &Mouse, win: Win, may_start: bool) {
    let scale = app.scale();
    let to_points = |p: Pos2| Pos2::new(p.x * scale, p.y * scale);
    if let Some(pointer) = mouse
        .pos
        .filter(|_| may_start && mouse.pressed && app.hot.is_none())
    {
        app.hot = Some(Hot::WindowDrag(win));
        app.windows.begin_drag(win, to_points(pointer));
    }
    if app.hot == Some(Hot::WindowDrag(win)) {
        if let Some(pointer) = mouse.pos {
            app.windows
                .update_drag(ctx, to_points(pointer), !mouse.shift);
        }
        if mouse.released || !mouse.down {
            app.windows.end_drag();
            app.hot = None;
        }
    }
    // Only the window that owns the interaction may end it.
    if !mouse.down && app.hot.is_some_and(|h| h.owner() == win) {
        app.hot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitbox_from_position_tuple() {
        assert_eq!(HitBox::at((5, 7), 20, 10), HitBox::new(5, 7, 20, 10));
    }
}
