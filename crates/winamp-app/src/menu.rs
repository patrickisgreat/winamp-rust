//! Winamp-style popup menus. Each menu level is its own frameless,
//! always-on-top viewport so it can extend beyond the tiny skinned windows.

use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Stroke, Vec2, ViewportBuilder, ViewportId, WindowLevel};

use crate::actions::Cmd;

#[derive(Clone, Debug)]
pub enum Item {
    Action {
        label: String,
        cmd: Cmd,
        checked: Option<bool>,
        enabled: bool,
    },
    Sub {
        label: String,
        items: Vec<Item>,
    },
    Sep,
}

impl Item {
    pub fn action(label: &str, cmd: Cmd) -> Item {
        Item::Action {
            label: label.to_string(),
            cmd,
            checked: None,
            enabled: true,
        }
    }
    pub fn check(label: &str, cmd: Cmd, checked: bool) -> Item {
        Item::Action {
            label: label.to_string(),
            cmd,
            checked: Some(checked),
            enabled: true,
        }
    }
    pub fn disabled(label: &str) -> Item {
        Item::Action {
            label: label.to_string(),
            cmd: Cmd::Nop,
            checked: None,
            enabled: false,
        }
    }
    pub fn sub(label: &str, items: Vec<Item>) -> Item {
        Item::Sub {
            label: label.to_string(),
            items,
        }
    }
}

const ITEM_H: f32 = 20.0;
const SEP_H: f32 = 7.0;
const MENU_W: f32 = 230.0;
const PAD: f32 = 4.0;

fn item_height(item: &Item) -> f32 {
    if matches!(item, Item::Sep) {
        SEP_H
    } else {
        ITEM_H
    }
}

/// Outer size of one menu level.
fn menu_size(items: &[Item]) -> Vec2 {
    let height: f32 = items.iter().map(item_height).sum();
    Vec2::new(MENU_W, height + PAD * 2.0)
}

/// Vertical offset of item `index` inside its level.
fn item_offset(items: &[Item], index: usize) -> f32 {
    items[..index].iter().map(item_height).sum()
}

pub struct Menu {
    pub items: Vec<Item>,
    /// Screen position (points) of the top-left corner.
    pub pos: Pos2,
    open: Vec<usize>,
    frames: u32,
    /// When our app (menus or windows) was last focused or the pointer was last over a menu.
    last_active: Option<Instant>,
    /// A submenu change waiting for the pointer to settle: (depth, item or usize::MAX for "close", since).
    pending: Option<(usize, usize, Instant)>,
}

const OPEN_DELAY: Duration = Duration::from_millis(120);
const CLOSE_DELAY: Duration = Duration::from_millis(350);

impl Menu {
    pub fn new(items: Vec<Item>, pos: Pos2) -> Self {
        Self {
            items,
            pos,
            open: Vec::new(),
            frames: 0,
            last_active: None,
            pending: None,
        }
    }

    /// Show all open levels. Returns `Some(cmd)` when an item was chosen or
    /// `Some(Cmd::Nop)` when dismissed; `None` while still open.
    /// `parent_focused` says whether any of the application's own windows has
    /// focus; frameless popups don't always receive focus themselves.
    pub fn show(&mut self, ctx: &egui::Context, parent_focused: bool) -> Option<Cmd> {
        self.frames += 1;
        let mut result: Option<Cmd> = None;
        let mut any_focused = false;
        let mut any_hovered = false;
        let mut escape = false;
        let mut pos = self.pos;
        let mut depth = 0usize;
        let mut path: Vec<usize> = self.open.clone();
        let mut new_open: Vec<usize> = Vec::new();
        let mut items: &[Item] = &self.items;
        loop {
            let size = menu_size(items);
            let id = ViewportId::from_hash_of(("winamp-menu", depth));
            let builder = ViewportBuilder::default()
                .with_title("menu")
                .with_decorations(false)
                .with_resizable(false)
                .with_inner_size(size)
                .with_position(pos)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_taskbar(false)
                .with_active(true);
            let mut hovered_sub: Option<usize> = None;
            let mut next_pos = pos;
            let open_here = path.get(depth).copied();
            ctx.show_viewport_immediate(id, builder, |ctx, _class| {
                let (focused, esc, hovered) = ctx.input(|i| {
                    (
                        i.viewport().focused.unwrap_or(false),
                        i.key_pressed(egui::Key::Escape),
                        i.pointer.has_pointer(),
                    )
                });
                any_focused |= focused;
                any_hovered |= hovered;
                escape |= esc;
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(Color32::from_rgb(30, 30, 40))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(120, 120, 150)))
                            .inner_margin(PAD),
                    )
                    .show(ctx, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                        for (i, item) in items.iter().enumerate() {
                            match item {
                                Item::Sep => {
                                    let (r, _) = ui.allocate_exact_size(
                                        Vec2::new(MENU_W - PAD * 2.0, SEP_H),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().hline(
                                        r.x_range(),
                                        r.center().y,
                                        Stroke::new(1.0, Color32::from_rgb(90, 90, 110)),
                                    );
                                }
                                Item::Action {
                                    label,
                                    cmd,
                                    checked,
                                    enabled,
                                } => {
                                    let mark = match checked {
                                        Some(true) => "✓",
                                        _ => " ",
                                    };
                                    let text = format!(" {mark}  {label}");
                                    let color = if *enabled {
                                        Color32::from_rgb(235, 235, 245)
                                    } else {
                                        Color32::from_rgb(120, 120, 130)
                                    };
                                    let r = ui.add_sized(
                                        Vec2::new(MENU_W - PAD * 2.0, ITEM_H),
                                        egui::Button::new(egui::RichText::new(text).color(color))
                                            .frame(false)
                                            .fill(Color32::TRANSPARENT),
                                    );
                                    if r.hovered() && *enabled {
                                        ui.painter().rect_filled(
                                            r.rect,
                                            2.0,
                                            Color32::from_rgba_unmultiplied(100, 120, 200, 60),
                                        );
                                        hovered_sub = Some(usize::MAX);
                                    }
                                    if r.clicked() && *enabled {
                                        result = Some(cmd.clone());
                                    }
                                }
                                Item::Sub { label, .. } => {
                                    let text = format!("    {label}");
                                    let r = ui.add_sized(
                                        Vec2::new(MENU_W - PAD * 2.0, ITEM_H),
                                        egui::Button::new(
                                            egui::RichText::new(text)
                                                .color(Color32::from_rgb(235, 235, 245)),
                                        )
                                        .frame(false)
                                        .fill(Color32::TRANSPARENT),
                                    );
                                    ui.painter().text(
                                        Pos2::new(r.rect.right() - 10.0, r.rect.center().y),
                                        egui::Align2::CENTER_CENTER,
                                        "▶",
                                        egui::FontId::proportional(10.0),
                                        Color32::from_rgb(200, 200, 220),
                                    );
                                    let is_open = open_here == Some(i);
                                    if r.hovered() || r.clicked() {
                                        hovered_sub = Some(i);
                                    }
                                    if is_open || r.hovered() {
                                        ui.painter().rect_filled(
                                            r.rect,
                                            2.0,
                                            Color32::from_rgba_unmultiplied(100, 120, 200, 60),
                                        );
                                    }
                                    if is_open {
                                        next_pos = Pos2::new(
                                            pos.x + MENU_W - 4.0,
                                            pos.y + r.rect.top() - PAD,
                                        );
                                    }
                                }
                            }
                        }
                    });
            });
            // Decide which submenu is open at this depth, with hysteresis so that
            // moving the pointer diagonally into a submenu (crossing neighbouring
            // items on the way) doesn't close it, like native menus.
            let now = Instant::now();
            let mut chosen = open_here;
            match hovered_sub {
                Some(i) if i != usize::MAX => {
                    if open_here.is_none() {
                        chosen = Some(i);
                        self.pending = None;
                    } else if open_here != Some(i) {
                        match self.pending {
                            Some((d, j, since)) if d == depth && j == i => {
                                if now.duration_since(since) > OPEN_DELAY {
                                    chosen = Some(i);
                                    self.pending = None;
                                }
                            }
                            _ => self.pending = Some((depth, i, now)),
                        }
                    } else {
                        self.pending = None;
                    }
                }
                Some(_) => {
                    // Hovering a plain item: close the open submenu after a delay.
                    if open_here.is_some() {
                        match self.pending {
                            Some((d, j, since)) if d == depth && j == usize::MAX => {
                                if now.duration_since(since) > CLOSE_DELAY {
                                    chosen = None;
                                    self.pending = None;
                                }
                            }
                            _ => self.pending = Some((depth, usize::MAX, now)),
                        }
                    }
                }
                None => {
                    // Pointer is elsewhere (likely inside the submenu): keep things as they are.
                    if matches!(self.pending, Some((d, _, _)) if d == depth) {
                        self.pending = None;
                    }
                }
            }
            if let Some(i) = chosen {
                new_open.push(i);
                if let Some(Item::Sub { items: sub, .. }) = items.get(i) {
                    // Recompute position if this submenu was just opened this frame.
                    if open_here != Some(i) {
                        next_pos = Pos2::new(pos.x + MENU_W - 4.0, pos.y + item_offset(items, i));
                    }
                    items = sub;
                    pos = next_pos;
                    depth += 1;
                    if depth > 4 {
                        break;
                    }
                    path = new_open.clone();
                    continue;
                }
            }
            break;
        }
        self.open = new_open;
        if result.is_some() {
            return result;
        }
        if escape {
            return Some(Cmd::Nop);
        }
        // Dismiss when neither a menu level nor one of our windows has focus
        // (the user switched to another application). Clicks on our own windows
        // dismiss the menu from the window side.
        // Frameless popups don't reliably report focus on every platform, so the
        // pointer being over a menu also counts as activity, and we only give up
        // after a generous timeout.
        let now = Instant::now();
        if any_focused || parent_focused || any_hovered || self.last_active.is_none() {
            self.last_active = Some(now);
        }
        if self.frames > 15 {
            if let Some(t) = self.last_active {
                if now.duration_since(t) > Duration::from_millis(1500) {
                    return Some(Cmd::Nop);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_size_counts_separators_shorter_than_items() {
        let items = vec![
            Item::action("a", Cmd::Nop),
            Item::Sep,
            Item::sub("s", vec![]),
        ];
        assert_eq!(
            menu_size(&items),
            Vec2::new(MENU_W, ITEM_H * 2.0 + SEP_H + PAD * 2.0)
        );
        assert_eq!(item_offset(&items, 2), ITEM_H + SEP_H);
        assert_eq!(menu_size(&[]), Vec2::new(MENU_W, PAD * 2.0));
    }

    #[test]
    fn item_constructors() {
        assert!(matches!(
            Item::check("x", Cmd::Nop, true),
            Item::Action {
                checked: Some(true),
                enabled: true,
                ..
            }
        ));
        assert!(matches!(
            Item::disabled("x"),
            Item::Action {
                cmd: Cmd::Nop,
                enabled: false,
                ..
            }
        ));
        assert!(
            matches!(Item::sub("x", vec![Item::Sep]), Item::Sub { items, .. } if items.len() == 1)
        );
    }
}
