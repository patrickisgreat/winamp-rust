//! The playlist editor window.

use egui::{Color32, Pos2, Rect, Vec2};
use winamp_audio::format_duration;
use winamp_playlist::SortKey;
use winamp_skin::sprites::*;

use crate::actions::Cmd;
use crate::app::{App, Hot};
use crate::gfx::{rgb, Gfx, Mouse};
use crate::main_window::{main_menu_items, open_menu_at};
use crate::menu::Item;
use crate::widgets::hit_button;
use crate::windows::Win;

const B_CLOSE: u16 = 1;
const B_SHADE: u16 = 2;
const B_ADD: u16 = 3;
const B_REM: u16 = 4;
const B_SEL: u16 = 5;
const B_MISC: u16 = 6;
const B_LIST: u16 = 7;

pub fn show(app: &mut App, ctx: &egui::Context) {
    app.windows.observe(Win::Pl, ctx);
    let scale = app.scale();
    let shaded = app.windows.get(Win::Pl).shaded;
    let focused = app.windows.get(Win::Pl).focused || app.menu.as_ref().map(|(w, _)| *w == Win::Pl).unwrap_or(false);
    let size = app.pl_size();
    let (w, h) = (size.x, if shaded { 14.0 } else { size.y });
    let mut cmds: Vec<Cmd> = Vec::new();
    let mut open_menu: Option<(Pos2, Vec<Item>)> = None;

    if ctx.input(|i| i.viewport().close_requested()) {
        cmds.push(Cmd::TogglePlaylist);
    }

    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let painter = ui.painter().clone();
        let m = Mouse::read(ctx, scale);
        let menu_open = app.menu.is_some();
        if menu_open && (m.pressed || m.right_pressed) {
            app.menu = None;
        }
        let m = if menu_open { Mouse { pos: None, pressed: false, down: false, double_clicked: false, right_pressed: false, ..m } } else { m };
        let textures = app.textures.clone();
        let skin = app.skin.clone();
        let g = Gfx { painter: &painter, textures: &textures, skin: &skin, origin: Pos2::ZERO, scale };

        if shaded {
            draw_shade(app, &g, &m, focused, w, &mut cmds);
        } else {
            draw_frame(app, &g, &m, focused, w, h, &mut cmds, &mut open_menu);
            draw_list(app, ctx, &g, &m, w, h, &mut cmds);
        }

        // Window drag from chrome (not from the list area).
        let in_list = !shaded && m.over_f(12.0, 20.0, w - 32.0, h - 58.0);
        if m.pressed && app.hot.is_none() && m.pos.is_some() && !in_list {
            app.hot = Some(Hot::WindowDrag(Win::Pl));
            app.windows.begin_drag(Win::Pl, Pos2::new(m.pos.unwrap().x * scale, m.pos.unwrap().y * scale));
        }
        if app.hot == Some(Hot::WindowDrag(Win::Pl)) {
            if let Some(p) = m.pos {
                app.windows.update_drag(ctx, Pos2::new(p.x * scale, p.y * scale), !m.shift);
            }
            if m.released || !m.down {
                app.windows.end_drag();
                app.hot = None;
            }
        }
        if !m.down && app.hot.map(|h| h.owner() == Win::Pl).unwrap_or(false) {
            app.hot = None;
        }
        if m.right_pressed && m.pos.is_some() && !menu_open {
            let pos = m.pos.map(|p| Pos2::new(p.x * scale, p.y * scale)).unwrap();
            let items = if in_list { list_context_items(app) } else { main_menu_items(app) };
            open_menu = Some((pos, items));
        }
    });

    for c in cmds {
        app.exec(c, ctx);
    }
    if let Some((pos, items)) = open_menu {
        open_menu_at(app, Win::Pl, pos, items);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(app: &mut App, g: &Gfx, m: &Mouse, focused: bool, w: f32, h: f32, cmds: &mut Vec<Cmd>, open_menu: &mut Option<(Pos2, Vec<Item>)>) {
    let scale = g.scale;
    // Top row.
    let (tl, title, tile, tr) = if focused { (PL_TL, PL_TITLE, PL_TOP_TILE, PL_TR) } else { (PL_TL_INACTIVE, PL_TITLE_INACTIVE, PL_TOP_TILE_INACTIVE, PL_TR_INACTIVE) };
    g.sprite(tl, 0.0, 0.0);
    g.tile_h(tile, 25.0, 0.0, w - 50.0);
    let title_x = ((w - 100.0) / 2.0).floor();
    g.sprite(title, title_x, 0.0);
    g.sprite(tr, w - 25.0, 0.0);
    // Sides.
    g.tile_v(PL_LEFT_TILE, 0.0, 20.0, h - 58.0);
    g.tile_v(PL_RIGHT_TILE, w - 20.0, 20.0, h - 58.0);
    // Bottom.
    g.sprite(PL_BL, 0.0, h - 38.0);
    g.tile_h(PL_BOTTOM_TILE, 125.0, h - 38.0, w - 275.0);
    g.sprite(PL_BR, w - 150.0, h - 38.0);

    // Title bar buttons.
    let bx = (w - 25.0) as u32;
    {
        let (clicked, down) = hit_button(m, &mut app.hot, Win::Pl, B_SHADE, bx + 5, 3, 9, 9);
        if down {
            g.sprite(PL_SHADE_DOWN, (bx + 5) as f32, 3.0);
        }
        if clicked {
            cmds.push(Cmd::TogglePlShade);
        }
        let (clicked, down) = hit_button(m, &mut app.hot, Win::Pl, B_CLOSE, bx + 15, 3, 9, 9);
        if down {
            g.sprite(PL_CLOSE_DOWN, (bx + 15) as f32, 3.0);
        }
        if clicked {
            cmds.push(Cmd::TogglePlaylist);
        }
    }
    if m.double_clicked && m.pos.map(|p| p.y < 20.0).unwrap_or(false) {
        cmds.push(Cmd::TogglePlShade);
    }

    // Bottom buttons -> popup menus (opened above the button, like Winamp).
    let by = (h - 38.0) as u32;
    let buttons = [
        (B_ADD, PL_BTN_ADD, 0u32),
        (B_REM, PL_BTN_REM, 0),
        (B_SEL, PL_BTN_SEL, 0),
        (B_MISC, PL_BTN_MISC, 0),
        (B_LIST, PL_BTN_LIST_FROM_RIGHT, 1),
    ];
    for (id, (bx0, dy, bw, bh), from_right) in buttons {
        let x = if from_right == 1 { (w - 150.0) as u32 + 150 - bx0 } else { bx0 };
        let (clicked, _) = hit_button(m, &mut app.hot, Win::Pl, id, x, by + dy, bw, bh);
        if clicked {
            let items = match id {
                B_ADD => vec![Item::action("Add URL...", Cmd::AddUrl), Item::action("Add dir...", Cmd::AddDirectory), Item::action("Add file(s)...", Cmd::AddFiles)],
                B_REM => vec![
                    Item::sub("Remove misc", vec![Item::action("Remove dead files", Cmd::PlRemoveMissing), Item::action("Remove duplicate entries", Cmd::PlRemoveDuplicates)]),
                    Item::action("Remove all", Cmd::PlRemoveAll),
                    Item::action("Crop selection", Cmd::PlCrop),
                    Item::action("Remove selected", Cmd::PlRemoveSelected),
                ],
                B_SEL => vec![Item::action("Invert selection", Cmd::PlInvertSelection), Item::action("Select none", Cmd::PlSelectNone), Item::action("Select all", Cmd::PlSelectAll)],
                B_MISC => vec![
                    Item::sub(
                        "Sort list",
                        vec![
                            Item::action(crate::app::sort_key_label(SortKey::Title), Cmd::PlSort(SortKey::Title)),
                            Item::action(crate::app::sort_key_label(SortKey::Filename), Cmd::PlSort(SortKey::Filename)),
                            Item::action(crate::app::sort_key_label(SortKey::Path), Cmd::PlSort(SortKey::Path)),
                            Item::Sep,
                            Item::action("Reverse list", Cmd::PlReverse),
                            Item::action("Randomize list", Cmd::PlRandomize),
                        ],
                    ),
                    Item::action("File info...", Cmd::PlFileInfo),
                    Item::action("Jump to file...", Cmd::JumpToFile),
                    Item::action("Playlist preferences...", Cmd::Preferences),
                ],
                _ => vec![Item::action("New list", Cmd::PlNew), Item::action("Save list...", Cmd::PlSave), Item::action("Load list...", Cmd::PlLoad)],
            };
            let n = items.len() as f32;
            let pos = Pos2::new(x as f32 * scale, ((by + dy) as f32 * scale) - n * 19.0 - 8.0);
            *open_menu = Some((pos, items));
        }
    }

    // Running time display in the bottom-right panel.
    let (sel, sel_known) = app.playlist.selected_duration();
    let (total, total_known) = app.playlist.total_duration();
    let text = format!(
        "{}{}/{}{}",
        format_duration(sel),
        if sel_known { "" } else { "+" },
        format_duration(total),
        if total_known { "" } else { "+" }
    );
    let col = rgb(app.skin.pledit.normal);
    g.text_clipped(w - 150.0 + 12.0, h - 38.0 + 25.0, &text, w - 150.0 + 11.0, 84.0);
    let _ = col;

    // Resize handle (bottom-right 20x20).
    {
        let over = m.over_f(w - 20.0, h - 20.0, 20.0, 20.0);
        if m.pressed && over && app.hot.is_none() {
            app.hot = Some(Hot::PlResize(Vec2::new(app.cfg.pl_extra_cols as f32, app.cfg.pl_extra_rows as f32)));
        }
        if let Some(Hot::PlResize(_)) = app.hot {
            if let Some(p) = m.pos {
                let cols = ((p.x + 10.0 - 275.0) / 25.0).floor().max(0.0) as u32;
                let rows = ((p.y + 10.0 - 116.0) / 29.0).floor().max(0.0) as u32;
                app.cfg.pl_extra_cols = cols.min(40);
                app.cfg.pl_extra_rows = rows.min(40);
            }
        }
    }
}

fn draw_shade(app: &mut App, g: &Gfx, m: &Mouse, focused: bool, w: f32, cmds: &mut Vec<Cmd>) {
    g.sprite(PL_SHADE_BG_LEFT, 0.0, 0.0);
    g.tile_h(PL_SHADE_BG, 25.0, 0.0, w - 75.0);
    g.sprite(if focused { PL_SHADE_BG_RIGHT_ACTIVE } else { PL_SHADE_BG_RIGHT }, w - 50.0, 0.0);
    let text = app.marquee_text();
    let max_chars = ((w - 70.0) / 5.0) as usize;
    let shown: String = text.chars().take(max_chars).collect();
    g.text_clipped(5.0, 4.0, &shown, 4.0, w - 70.0);
    let bx = (w - 50.0) as u32;
    {
        let (clicked, down) = hit_button(m, &mut app.hot, Win::Pl, B_SHADE, bx + 30, 3, 9, 9);
        if down {
            g.sprite(PL_SHADE_UNSHADE_DOWN, (bx + 30) as f32, 3.0);
        }
        if clicked {
            cmds.push(Cmd::TogglePlShade);
        }
        let (clicked, _) = hit_button(m, &mut app.hot, Win::Pl, B_CLOSE, bx + 40, 3, 9, 9);
        if clicked {
            cmds.push(Cmd::TogglePlaylist);
        }
    }
    if m.double_clicked && m.pos.map(|p| p.x < w - 50.0).unwrap_or(false) {
        cmds.push(Cmd::TogglePlShade);
    }
}

fn draw_list(app: &mut App, ctx: &egui::Context, g: &Gfx, m: &Mouse, w: f32, h: f32, cmds: &mut Vec<Cmd>) {
    let scale = g.scale;
    let list = Rect::from_min_size(Pos2::new(12.0, 20.0), Vec2::new(w - 32.0, h - 58.0));
    let row_h = app.pl_row_h();
    let rows = app.pl_visible_rows();
    let n = app.playlist.len();
    let max_scroll = n.saturating_sub(rows);
    if app.pl_scroll > max_scroll {
        app.pl_scroll = max_scroll;
    }
    // Mouse wheel.
    if m.scroll != 0.0 && m.over_f(list.min.x, list.min.y, list.width(), list.height()) {
        let step = 3i64;
        let d = if m.scroll > 0.0 { -step } else { step };
        app.pl_scroll = (app.pl_scroll as i64 + d).clamp(0, max_scroll as i64) as usize;
    }

    let colors = &app.skin.pledit;
    g.fill(list.min.x, list.min.y, list.width(), list.height(), rgb(colors.normal_bg));
    let clip = g.rect(list.min.x, list.min.y, list.width(), list.height());
    let clipped = g.painter.with_clip_rect(clip);
    let gc = Gfx { painter: &clipped, textures: g.textures, skin: g.skin, origin: g.origin, scale };
    let font = app.cfg.playlist_font_size;
    let drag_target = match app.hot {
        Some(Hot::PlDragRows(_)) => m.pos.map(|p| (((p.y - list.min.y) / row_h).round() as i64 + app.pl_scroll as i64).clamp(0, n as i64) as usize),
        _ => None,
    };
    for r in 0..rows + 1 {
        let i = app.pl_scroll + r;
        if i >= n {
            break;
        }
        let y = list.min.y + r as f32 * row_h;
        let e = &app.playlist.entries[i];
        if e.selected {
            gc.fill(list.min.x, y, list.width(), row_h, rgb(colors.selected_bg));
        }
        let col = if Some(i) == app.playlist.current { rgb(colors.current) } else { rgb(colors.normal) };
        let label = format!("{}. {}", i + 1, e.title);
        let dur = e.duration.map(format_duration).unwrap_or_default();
        let dur_w = dur.len() as f32 * font * 0.6 + 4.0;
        let text_clip = gc.rect(list.min.x, y, list.width() - dur_w - 4.0, row_h);
        let tp = clipped.with_clip_rect(text_clip);
        let gt = Gfx { painter: &tp, textures: g.textures, skin: g.skin, origin: g.origin, scale };
        gt.egui_text(list.min.x + 2.0, y + 1.0, &label, font, col);
        gc.egui_text_right(list.max.x - 2.0, y + 1.0, &dur, font, col);
        if drag_target == Some(i) {
            gc.fill(list.min.x, y, list.width(), 1.0, Color32::WHITE);
        }
    }
    if drag_target == Some(n) && n > 0 {
        let y = list.min.y + (n - app.pl_scroll) as f32 * row_h;
        gc.fill(list.min.x, y, list.width(), 1.0, Color32::WHITE);
    }

    // Scrollbar.
    let track_y = 20.0;
    let track_h = h - 58.0;
    let thumb_travel = track_h - 18.0;
    let thumb_y = if max_scroll > 0 { track_y + (app.pl_scroll as f32 / max_scroll as f32) * thumb_travel } else { track_y };
    let sx = w - 15.0;
    let dragging_scroll = matches!(app.hot, Some(Hot::PlScroll(_)));
    g.sprite(if dragging_scroll { PL_SCROLL_THUMB_DOWN } else { PL_SCROLL_THUMB }, sx, thumb_y.round());
    if m.pressed && app.hot.is_none() && m.over_f(sx - 2.0, track_y, 12.0, track_h) {
        let grab = m.pos.map(|p| if p.y >= thumb_y && p.y < thumb_y + 18.0 { p.y - thumb_y } else { 9.0 }).unwrap_or(9.0);
        app.hot = Some(Hot::PlScroll(grab));
    }
    if let Some(Hot::PlScroll(grab)) = app.hot {
        if let Some(p) = m.pos {
            if max_scroll > 0 && thumb_travel > 0.0 {
                let t = ((p.y - grab - track_y) / thumb_travel).clamp(0.0, 1.0);
                app.pl_scroll = (t * max_scroll as f32).round() as usize;
            }
        }
    }

    // List interaction: select / play / drag-reorder.
    let over_list = m.over_f(list.min.x, list.min.y, list.width(), list.height());
    let row_at = |p: Pos2| -> Option<usize> {
        let r = ((p.y - list.min.y) / row_h).floor();
        if r < 0.0 {
            return None;
        }
        let i = app.pl_scroll + r as usize;
        if i < n {
            Some(i)
        } else {
            None
        }
    };
    if m.pressed && over_list && app.hot.is_none() {
        if let Some(p) = m.pos {
            match row_at(p) {
                Some(i) => {
                    if m.shift {
                        let anchor = app.pl_anchor.unwrap_or(i);
                        let (a, b) = if anchor <= i { (anchor, i) } else { (i, anchor) };
                        for (k, e) in app.playlist.entries.iter_mut().enumerate() {
                            e.selected = k >= a && k <= b;
                        }
                    } else if m.ctrl {
                        app.playlist.entries[i].selected = !app.playlist.entries[i].selected;
                        app.pl_anchor = Some(i);
                    } else {
                        if !app.playlist.entries[i].selected {
                            app.playlist.select_none();
                            app.playlist.entries[i].selected = true;
                        }
                        app.pl_anchor = Some(i);
                    }
                    app.hot = Some(Hot::PlDragRows(i));
                }
                None => {
                    if !m.ctrl && !m.shift {
                        app.playlist.select_none();
                    }
                    app.hot = Some(Hot::PlDragRows(usize::MAX));
                }
            }
        }
    }
    if m.double_clicked && over_list {
        if let Some(i) = m.pos.and_then(row_at) {
            cmds.push(Cmd::PlayIndex(i));
        }
    }
    if let Some(Hot::PlDragRows(start)) = app.hot {
        if m.released && start != usize::MAX {
            // A "drag" that ends on the same row (or the gap right below it) is a plain click.
            let is_click = drag_target.map(|t| t == start || t == start + 1).unwrap_or(true);
            if is_click {
                if !m.ctrl && !m.shift {
                    app.playlist.select_none();
                    if start < app.playlist.len() {
                        app.playlist.entries[start].selected = true;
                    }
                }
            } else if let Some(target) = drag_target {
                let sel = app.playlist.selected_indices();
                if !sel.is_empty() && (!sel.contains(&target) || target == n) {
                    app.playlist.move_entries(&sel, target);
                }
            }
        } else if m.down {
            // Auto-scroll when dragging past the edges.
            if let Some(p) = m.pos {
                if p.y < list.min.y && app.pl_scroll > 0 {
                    app.pl_scroll -= 1;
                } else if p.y > list.max.y && app.pl_scroll < max_scroll {
                    app.pl_scroll += 1;
                }
            }
        }
    }
    let _ = ctx;
}

pub fn list_context_items(app: &App) -> Vec<Item> {
    let has_sel = !app.playlist.selected_indices().is_empty();
    vec![
        Item::action("Play selected  (Enter)", Cmd::PlPlaySelected),
        Item::action("File info...  (Alt+3)", Cmd::PlFileInfo),
        Item::Sep,
        if has_sel { Item::action("Remove selected  (Del)", Cmd::PlRemoveSelected) } else { Item::disabled("Remove selected") },
        Item::action("Crop selection", Cmd::PlCrop),
        Item::action("Move up  (Alt+Up)", Cmd::PlMoveSelected(-1)),
        Item::action("Move down  (Alt+Down)", Cmd::PlMoveSelected(1)),
        Item::Sep,
        Item::action("Select all  (Ctrl+A)", Cmd::PlSelectAll),
        Item::action("Select none", Cmd::PlSelectNone),
        Item::action("Invert selection", Cmd::PlInvertSelection),
        Item::Sep,
        Item::action("Add file(s)...", Cmd::AddFiles),
        Item::action("Add dir...", Cmd::AddDirectory),
        Item::action("Add URL...", Cmd::AddUrl),
    ]
}

