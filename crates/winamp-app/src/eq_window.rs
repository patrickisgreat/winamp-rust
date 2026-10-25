//! The graphic equalizer window.

use egui::Pos2;
use winamp_audio::eq::MAX_GAIN_DB;
use winamp_audio::EqSettings;
use winamp_skin::sprites::*;
use winamp_skin::SkinImage;

use crate::actions::Cmd;
use crate::app::App;
use crate::gfx::{rgb, Gfx, Mouse};
use crate::main_window::{main_menu_items, open_menu_at};
use crate::widgets::{drag_window, ButtonState, HitBox, Widgets};
use crate::windows::Win;

const B_ON: u16 = 1;
const B_AUTO: u16 = 2;
const B_PRESETS: u16 = 3;
const B_CLOSE: u16 = 4;
const B_SHADE: u16 = 5;
const S_PREAMP: u16 = 10;
const S_BAND: u16 = 11; // +0..9

pub fn show(app: &mut App, ctx: &egui::Context) {
    app.windows.observe(Win::Eq, ctx);
    let scale = app.scale();
    let shaded = app.windows.get(Win::Eq).shaded;
    let focused = app.windows.get(Win::Eq).focused
        || app
            .menu
            .as_ref()
            .map(|(w, _)| *w == Win::Eq)
            .unwrap_or(false);
    let mut cmds: Vec<Cmd> = Vec::new();
    let mut open_menu: Option<Pos2> = None;

    if ctx.input(|i| i.viewport().close_requested()) {
        cmds.push(Cmd::ToggleEq);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let painter = ui.painter().clone();
            let m = Mouse::read(ctx, scale);
            let menu_open = app.menu.is_some();
            if menu_open && (m.pressed || m.right_pressed) {
                app.menu = None;
            }
            let m = if menu_open {
                Mouse {
                    pos: None,
                    pressed: false,
                    down: false,
                    double_clicked: false,
                    right_pressed: false,
                    ..m
                }
            } else {
                m
            };
            let textures = app.textures.clone();
            let skin = app.skin.clone();
            let g = Gfx {
                painter: &painter,
                textures: &textures,
                skin: &skin,
                origin: Pos2::ZERO,
                scale,
            };

            let shade_widgets = Widgets::new(&g, &m, Win::Eq);
            if shaded {
                g.sprite(
                    if focused {
                        EQ_SHADE_TITLE_ACTIVE
                    } else {
                        EQ_SHADE_TITLE_INACTIVE
                    },
                    0.0,
                    0.0,
                );
                if shade_widgets.button(
                    &mut app.hot,
                    B_SHADE,
                    POS_EQ_SHADE,
                    EQ_SHADE_UNSHADE,
                    EQ_SHADE_UNSHADE_DOWN,
                ) {
                    cmds.push(Cmd::ToggleEqShade);
                }
                if shade_widgets.button(
                    &mut app.hot,
                    B_CLOSE,
                    POS_EQ_CLOSE,
                    EQ_SHADE_CLOSE,
                    EQ_SHADE_CLOSE_DOWN,
                ) {
                    cmds.push(Cmd::ToggleEq);
                }
                if m.double_clicked && m.pos.map(|p| p.x < 240.0).unwrap_or(false) {
                    cmds.push(Cmd::ToggleEqShade);
                }
            } else {
                draw_full(app, ctx, &g, &m, focused, &mut cmds);
            }

            drag_window(app, ctx, &m, Win::Eq, true);
            if m.right_pressed && m.pos.is_some() && !menu_open {
                open_menu = m.pos.map(|p| Pos2::new(p.x * scale, p.y * scale));
            }
        });

    for c in cmds {
        app.exec(c, ctx);
    }
    if let Some(pos) = open_menu {
        let items = main_menu_items(app);
        open_menu_at(app, Win::Eq, pos, items);
    }
}

fn gain_to_t(db: f32) -> f32 {
    (db + MAX_GAIN_DB) / (2.0 * MAX_GAIN_DB)
}

fn t_to_gain(t: f32) -> f32 {
    (t * 2.0 * MAX_GAIN_DB - MAX_GAIN_DB).clamp(-MAX_GAIN_DB, MAX_GAIN_DB)
}

fn draw_full(
    app: &mut App,
    ctx: &egui::Context,
    g: &Gfx,
    m: &Mouse,
    focused: bool,
    cmds: &mut Vec<Cmd>,
) {
    let widgets = Widgets::new(g, m, Win::Eq);
    g.sprite(EQ_BG, 0.0, 0.0);
    g.sprite(
        if focused {
            EQ_TITLE_ACTIVE
        } else {
            EQ_TITLE_INACTIVE
        },
        0.0,
        0.0,
    );
    let eq: EqSettings = app.player.eq();

    // Title bar buttons (shade is baked into the title bar, close has a sprite).
    if widgets.button(&mut app.hot, B_CLOSE, POS_EQ_CLOSE, EQ_CLOSE, EQ_CLOSE_DOWN) {
        cmds.push(Cmd::ToggleEq);
    }
    {
        let ButtonState { clicked, .. } =
            widgets.hit_button(&mut app.hot, B_SHADE, HitBox::at(POS_EQ_SHADE, 9, 9));
        if clicked {
            cmds.push(Cmd::ToggleEqShade);
        }
    }
    // ON / AUTO / PRESETS
    if widgets.button(
        &mut app.hot,
        B_ON,
        POS_EQ_ON,
        if eq.enabled { EQ_ON_ON } else { EQ_ON_OFF },
        if eq.enabled {
            EQ_ON_ON_DOWN
        } else {
            EQ_ON_OFF_DOWN
        },
    ) {
        cmds.push(Cmd::ToggleEqEnabled);
    }
    let auto = app.eq_auto;
    if widgets.button(
        &mut app.hot,
        B_AUTO,
        POS_EQ_AUTO,
        if auto { EQ_AUTO_ON } else { EQ_AUTO_OFF },
        if auto {
            EQ_AUTO_ON_DOWN
        } else {
            EQ_AUTO_OFF_DOWN
        },
    ) {
        cmds.push(Cmd::ToggleEqAuto);
    }
    if widgets.button(
        &mut app.hot,
        B_PRESETS,
        POS_EQ_PRESETS,
        EQ_PRESETS,
        EQ_PRESETS_DOWN,
    ) {
        cmds.push(Cmd::EqPresetsDialog);
    }

    // Sliders.
    let draw_slider = |app: &mut App, id: u16, x: u32, y: u32, value_db: f32| -> Option<f32> {
        let t = gain_to_t(value_db);
        let r = widgets.vslider(&mut app.hot, id, HitBox::new(x, y, 14, 63), 11, t);
        let shown_t = if r.dragging { r.value } else { t };
        let frame = (shown_t * (EQ_SLIDER_FRAMES - 1) as f32).round() as u32;
        g.sprite(eq_slider_bg(frame), x as f32, y as f32);
        let ty = y as f32 + ((1.0 - shown_t) * EQ_SLIDER_TRAVEL as f32).round() + 1.0;
        g.sprite(
            if r.dragging { EQ_THUMB_DOWN } else { EQ_THUMB },
            x as f32 + 1.0,
            ty,
        );
        if r.dragging {
            let mut db = t_to_gain(r.value);
            if db.abs() < 0.6 {
                db = 0.0; // centre detent
            }
            if (db - value_db).abs() > 1e-3 {
                return Some(db);
            }
        }
        None
    };
    if let Some(v) = draw_slider(
        app,
        S_PREAMP,
        POS_EQ_PREAMP.0,
        POS_EQ_PREAMP.1,
        eq.preamp_db,
    ) {
        cmds.push(Cmd::SetPreamp(v));
        app.set_status(format!("EQ: preamp {:+.1} dB", v));
    }
    for i in 0..10u32 {
        let (x, y) = pos_eq_band(i);
        if let Some(v) = draw_slider(app, S_BAND + i as u16, x, y, eq.bands_db[i as usize]) {
            cmds.push(Cmd::SetBand(i as usize, v));
            app.set_status(format!(
                "EQ: {} Hz {:+.1} dB",
                winamp_audio::BAND_LABELS[i as usize],
                v
            ));
        }
    }

    // Graph.
    draw_graph(app, ctx, g, &eq);
}

fn draw_graph(app: &mut App, ctx: &egui::Context, g: &Gfx, eq: &EqSettings) {
    let surf = &mut app.vis.eq_graph;
    // Background from the skin sprite.
    let bg = app.skin.image(SkinImage::EqMain);
    for y in 0..19u32 {
        for x in 0..113u32 {
            let (sx, sy) = (EQ_GRAPH_BG.x + x, EQ_GRAPH_BG.y + y);
            let c = if sx < bg.width() && sy < bg.height() {
                let p = bg.get_pixel(sx, sy);
                egui::Color32::from_rgb(p[0], p[1], p[2])
            } else {
                egui::Color32::BLACK
            };
            surf.px(x as i32, y as i32, c);
        }
    }
    // Catmull-Rom interpolated curve through the 10 band gains.
    let pts: Vec<f32> = eq
        .bands_db
        .iter()
        .map(|d| (1.0 - gain_to_t(*d)) * 18.0)
        .collect();
    let mut prev: Option<i32> = None;
    for x in 0..113 {
        let fx = x as f32 / 112.0 * 9.0;
        let i = fx.floor() as usize;
        let t = fx - i as f32;
        let p = |k: isize| -> f32 { pts[k.clamp(0, 9) as usize] };
        let (p0, p1, p2, p3) = (
            p(i as isize - 1),
            p(i as isize),
            p(i as isize + 1),
            p(i as isize + 2),
        );
        let t2 = t * t;
        let t3 = t2 * t;
        let y = 0.5
            * ((2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
        let yi = y.round().clamp(0.0, 18.0) as i32;
        let color = rgb(app.skin.pixel(
            SkinImage::EqMain,
            EQ_GRAPH_COLORS.x,
            EQ_GRAPH_COLORS.y + yi as u32,
        ));
        match prev {
            Some(py) if (py - yi).abs() > 1 => {
                let (a, b) = if py < yi { (py + 1, yi) } else { (yi, py - 1) };
                for yy in a..=b {
                    let c = rgb(app.skin.pixel(
                        SkinImage::EqMain,
                        EQ_GRAPH_COLORS.x,
                        EQ_GRAPH_COLORS.y + yy as u32,
                    ));
                    surf.px(x, yy, c);
                }
            }
            _ => surf.px(x, yi, color),
        }
        prev = Some(yi);
    }
    surf.draw(ctx, g, POS_EQ_GRAPH.0 as f32, POS_EQ_GRAPH.1 as f32);
}
