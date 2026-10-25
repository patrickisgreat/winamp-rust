//! Winamp-style popup menus. Each menu level is its own frameless,
//! always-on-top viewport so it can extend beyond the tiny skinned windows.

use egui::{Color32, Pos2, Stroke, Vec2, ViewportBuilder, ViewportId, WindowLevel};

use crate::actions::Cmd;

#[derive(Clone, Debug)]
pub enum Item {
    Action { label: String, cmd: Cmd, checked: Option<bool>, enabled: bool },
    Sub { label: String, items: Vec<Item> },
    Sep,
}

impl Item {
    pub fn action(label: &str, cmd: Cmd) -> Item {
        Item::Action { label: label.to_string(), cmd, checked: None, enabled: true }
    }
    pub fn check(label: &str, cmd: Cmd, checked: bool) -> Item {
        Item::Action { label: label.to_string(), cmd, checked: Some(checked), enabled: true }
    }
    pub fn disabled(label: &str) -> Item {
        Item::Action { label: label.to_string(), cmd: Cmd::Nop, checked: None, enabled: false }
    }
    pub fn sub(label: &str, items: Vec<Item>) -> Item {
        Item::Sub { label: label.to_string(), items }
    }
}

const ITEM_H: f32 = 20.0;
const SEP_H: f32 = 7.0;
const MENU_W: f32 = 230.0;
const PAD: f32 = 4.0;

fn menu_size(items: &[Item]) -> Vec2 {
    let h: f32 = items.iter().map(|i| if matches!(i, Item::Sep) { SEP_H } else { ITEM_H }).sum();
    Vec2::new(MENU_W, h + PAD * 2.0)
}

pub struct Menu {
    pub items: Vec<Item>,
    /// Screen position (points) of the top-left corner.
    pub pos: Pos2,
    open: Vec<usize>,
    frames: u32,
    unfocused_frames: u32,
}

impl Menu {
    pub fn new(items: Vec<Item>, pos: Pos2) -> Self {
        Self { items, pos, open: Vec::new(), frames: 0, unfocused_frames: 0 }
    }

    /// Show all open levels. Returns `Some(cmd)` when an item was chosen or
    /// `Some(Cmd::Nop)` when dismissed; `None` while still open.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Cmd> {
        self.frames += 1;
        let mut result: Option<Cmd> = None;
        let mut any_focused = false;
        let mut escape = false;
        let mut pos = self.pos;
        let mut depth = 0usize;
        let mut path: Vec<usize> = self.open.clone();
        let mut new_open: Vec<usize> = Vec::new();
        let mut items: Vec<Item> = self.items.clone();
        loop {
            let size = menu_size(&items);
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
                let (focused, esc) = ctx.input(|i| (i.viewport().focused.unwrap_or(false), i.key_pressed(egui::Key::Escape)));
                any_focused |= focused;
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
                                    let (r, _) = ui.allocate_exact_size(Vec2::new(MENU_W - PAD * 2.0, SEP_H), egui::Sense::hover());
                                    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, Color32::from_rgb(90, 90, 110)));
                                }
                                Item::Action { label, cmd, checked, enabled } => {
                                    let mark = match checked {
                                        Some(true) => "✓",
                                        _ => " ",
                                    };
                                    let text = format!(" {mark}  {label}");
                                    let color = if *enabled { Color32::from_rgb(235, 235, 245) } else { Color32::from_rgb(120, 120, 130) };
                                    let r = ui.add_sized(
                                        Vec2::new(MENU_W - PAD * 2.0, ITEM_H),
                                        egui::Button::new(egui::RichText::new(text).color(color)).frame(false).fill(Color32::TRANSPARENT),
                                    );
                                    if r.hovered() && *enabled {
                                        ui.painter().rect_filled(r.rect, 2.0, Color32::from_rgba_unmultiplied(100, 120, 200, 60));
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
                                        egui::Button::new(egui::RichText::new(text).color(Color32::from_rgb(235, 235, 245))).frame(false).fill(Color32::TRANSPARENT),
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
                                        ui.painter().rect_filled(r.rect, 2.0, Color32::from_rgba_unmultiplied(100, 120, 200, 60));
                                    }
                                    if is_open {
                                        next_pos = Pos2::new(pos.x + MENU_W - 4.0, pos.y + r.rect.top() - PAD);
                                    }
                                }
                            }
                        }
                    });
            });
            // Decide which submenu is open at this depth.
            let chosen = match hovered_sub {
                Some(usize::MAX) => None,                 // hovering a plain item closes submenus
                Some(i) => Some(i),
                None => open_here,                         // keep what was open
            };
            if let Some(i) = chosen {
                new_open.push(i);
                if let Some(Item::Sub { items: sub, .. }) = items.get(i) {
                    // Recompute position if this submenu was just opened this frame.
                    if open_here != Some(i) {
                        let y_off: f32 = items[..i].iter().map(|it| if matches!(it, Item::Sep) { SEP_H } else { ITEM_H }).sum();
                        next_pos = Pos2::new(pos.x + MENU_W - 4.0, pos.y + y_off);
                    }
                    items = sub.clone();
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
        // Dismiss when no menu level has focus (user clicked elsewhere).
        if self.frames > 15 {
            if any_focused {
                self.unfocused_frames = 0;
            } else {
                self.unfocused_frames += 1;
                if self.unfocused_frames > 3 {
                    return Some(Cmd::Nop);
                }
            }
        }
        None
    }
}
