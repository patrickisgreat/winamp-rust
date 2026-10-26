//! The main Winamp window (full and windowshade modes).

use std::time::Duration;

use egui::Pos2;
use winamp_audio::{format_duration, PlayState};
use winamp_skin::sprites::*;

use crate::actions::Cmd;
use crate::app::{App, Hot};
use crate::config::{AnalyzerStyle, TimeMode, VisMode};
use crate::gfx::{rgb, Gfx, Mouse};
use crate::menu::{Item, Menu};
use crate::widgets::{drag_window, ButtonState, HitBox, Widgets};
use crate::windows::Win;

// control ids
const B_PREV: u16 = 1;
const B_PLAY: u16 = 2;
const B_PAUSE: u16 = 3;
const B_STOP: u16 = 4;
const B_NEXT: u16 = 5;
const B_EJECT: u16 = 6;
const B_SHUFFLE: u16 = 7;
const B_REPEAT: u16 = 8;
const B_EQ: u16 = 9;
const B_PL: u16 = 10;
const B_OPTIONS: u16 = 11;
const B_MINIMIZE: u16 = 12;
const B_SHADE: u16 = 13;
const B_CLOSE: u16 = 14;
const B_CLUTTER: u16 = 20; // +0..4
const B_TIME: u16 = 30;
const B_VIS: u16 = 31;
const S_POS: u16 = 40;
const S_VOL: u16 = 41;
const S_BAL: u16 = 42;

pub fn show(app: &mut App, ctx: &egui::Context) {
    let scale = app.scale();
    let shaded = app.windows.get(Win::Main).shaded;
    let focused = app.windows.get(Win::Main).focused
        || app
            .menu
            .as_ref()
            .map(|(w, _)| *w == Win::Main)
            .unwrap_or(false);
    let mut cmds: Vec<Cmd> = Vec::new();
    let mut open_menu: Option<Pos2> = None;
    let mut vis_menu: Option<Pos2> = None;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let painter = ui.painter().clone();
            let m = Mouse::read(ctx, scale);
            let textures = app.textures.clone();
            let skin = app.skin.clone();
            let g = Gfx {
                painter: &painter,
                textures: &textures,
                skin: &skin,
                origin: Pos2::ZERO,
                scale,
            };
            let menu_open = app.menu.is_some();
            let raw_pressed = m.pressed || m.right_pressed;
            let m = if menu_open {
                Mouse {
                    pos: None,
                    pressed: false,
                    released: m.released,
                    down: false,
                    double_clicked: false,
                    right_pressed: false,
                    ..m
                }
            } else {
                m
            };

            if shaded {
                draw_shade(app, &g, &m, focused, &mut cmds, &mut open_menu);
            } else {
                draw_full(
                    app,
                    ctx,
                    &g,
                    &m,
                    focused,
                    &mut cmds,
                    &mut open_menu,
                    &mut vis_menu,
                );
            }

            drag_window(app, ctx, &m, Win::Main, true);
            if m.right_pressed && m.pos.is_some() && !menu_open {
                open_menu = m.pos.map(|p| Pos2::new(p.x * scale, p.y * scale));
            }
            if menu_open && raw_pressed {
                app.menu = None;
            }
            // Mouse wheel adjusts volume, as in Winamp.
            if m.double_clicked
                && m.pos
                    .map(|p| p.y < 14.0 && p.x > 16.0 && p.x < 240.0)
                    .unwrap_or(false)
            {
                cmds.push(Cmd::ToggleMainShade);
            }
            if m.scroll != 0.0 && !menu_open {
                cmds.push(Cmd::VolumeDelta(if m.scroll > 0.0 { 0.02 } else { -0.02 }));
            }
        });

    for c in cmds {
        app.exec(c, ctx);
    }
    if let Some(pos) = vis_menu {
        open_menu_at(app, Win::Main, pos, vis_menu_items(app));
    } else if let Some(pos) = open_menu {
        open_menu_at(app, Win::Main, pos, main_menu_items(app));
    }
}

/// Open a popup menu at a window-relative position (points) of `win`.
pub fn open_menu_at(app: &mut App, win: Win, pos: Pos2, items: Vec<Item>) {
    let origin = app
        .windows
        .get(win)
        .rect
        .map(|r| r.min)
        .unwrap_or(Pos2::ZERO);
    app.menu = Some((win, Menu::new(items, origin + pos.to_vec2())));
}

/// Render and process the active popup menu, if any (called once per frame
/// from the top-level update, outside any child viewport).
pub fn show_menu(app: &mut App, ctx: &egui::Context) {
    let mut done: Option<Cmd> = None;
    let parent_focused = app.windows.info.iter().any(|w| w.visible && w.focused);
    if let Some((_, menu)) = &mut app.menu {
        done = menu.show(ctx, parent_focused);
    }
    if let Some(cmd) = done {
        app.menu = None;
        app.exec(cmd, ctx);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_full(
    app: &mut App,
    ctx: &egui::Context,
    g: &Gfx,
    m: &Mouse,
    focused: bool,
    cmds: &mut Vec<Cmd>,
    open_menu: &mut Option<Pos2>,
    vis_menu: &mut Option<Pos2>,
) {
    let scale = g.scale;
    let widgets = Widgets::new(g, m, Win::Main);
    g.sprite(MAIN_BG, 0.0, 0.0);
    g.sprite(
        if focused {
            TITLE_ACTIVE
        } else {
            TITLE_INACTIVE
        },
        0.0,
        0.0,
    );

    // Title bar buttons.
    let hot = &mut app.hot;
    if widgets.button(
        hot,
        B_OPTIONS,
        POS_BTN_OPTIONS,
        BTN_OPTIONS,
        BTN_OPTIONS_DOWN,
    ) {
        *open_menu = Some(Pos2::new(6.0 * scale, 14.0 * scale));
    }
    if widgets.button(
        hot,
        B_MINIMIZE,
        POS_BTN_MINIMIZE,
        BTN_MINIMIZE,
        BTN_MINIMIZE_DOWN,
    ) {
        cmds.push(Cmd::MinimizeAll);
    }
    if widgets.button(hot, B_SHADE, POS_BTN_SHADE, BTN_SHADE, BTN_SHADE_DOWN) {
        cmds.push(Cmd::ToggleMainShade);
    }
    if widgets.button(hot, B_CLOSE, POS_BTN_CLOSE, BTN_CLOSE, BTN_CLOSE_DOWN) {
        cmds.push(Cmd::Exit);
    }

    // Clutter bar (O A I D V).
    g.sprite(CLUTTER_BAR, POS_CLUTTER.0 as f32, POS_CLUTTER.1 as f32);
    let downs = [
        CLUTTER_O_DOWN,
        CLUTTER_A_DOWN,
        CLUTTER_I_DOWN,
        CLUTTER_D_DOWN,
        CLUTTER_V_DOWN,
    ];
    for (i, (dy, h)) in CLUTTER_ROWS.iter().enumerate() {
        let (x, y) = (POS_CLUTTER.0, POS_CLUTTER.1 + dy);
        let ButtonState {
            clicked,
            held: down,
        } = widgets.hit_button(hot, B_CLUTTER + i as u16, HitBox::new(x, y, 8, *h));
        let lit = match i {
            1 => app.cfg.always_on_top,
            3 => app.cfg.double_size,
            _ => false,
        };
        if down || lit {
            g.sprite(downs[i], x as f32, y as f32);
        }
        if clicked {
            match i {
                0 => *open_menu = Some(Pos2::new(18.0 * scale, 22.0 * scale)),
                1 => cmds.push(Cmd::ToggleAlwaysOnTop),
                2 => cmds.push(Cmd::FileInfo),
                3 => cmds.push(Cmd::ToggleDoubleSize),
                4 => *vis_menu = Some(Pos2::new(18.0 * scale, 52.0 * scale)),
                _ => {}
            }
        }
    }

    // Play state indicator.
    let state = app.player.state();
    let ind = match state {
        PlayState::Playing => IND_PLAYING,
        PlayState::Paused => IND_PAUSED,
        PlayState::Stopped => IND_STOPPED,
    };
    g.sprite(ind, POS_INDICATOR.0 as f32, POS_INDICATOR.1 as f32);
    let working = app
        .status
        .as_ref()
        .map(|(s, _)| s.starts_with("Connecting"))
        .unwrap_or(false);
    g.sprite(
        if working { IND_WORK_ON } else { IND_WORK_OFF },
        POS_WORK_INDICATOR.0 as f32,
        POS_WORK_INDICATOR.1 as f32,
    );

    // Time display.
    draw_time(app, g, m, cmds);

    // Marquee.
    draw_marquee(app, g, m);

    // Bitrate / sample rate / mono-stereo.
    if let Some(info) = app.player.info() {
        if state != PlayState::Stopped || app.player.current_source().is_some() {
            if let Some(br) = info.bitrate_kbps {
                let s = if br > 9999 {
                    "9999".to_string()
                } else {
                    br.to_string()
                };
                g.text(POS_BITRATE.0 as f32, POS_BITRATE.1 as f32, &s);
            }
            let khz = (info.sample_rate as f32 / 1000.0).round() as u32;
            g.text(
                POS_SAMPLERATE.0 as f32,
                POS_SAMPLERATE.1 as f32,
                &format!("{khz:2}"),
            );
            let stereo = info.channels >= 2;
            g.sprite(
                if stereo { STEREO_ON } else { STEREO_OFF },
                POS_STEREO.0 as f32,
                POS_STEREO.1 as f32,
            );
            g.sprite(
                if stereo { MONO_OFF } else { MONO_ON },
                POS_MONO.0 as f32,
                POS_MONO.1 as f32,
            );
        }
    } else {
        g.sprite(STEREO_OFF, POS_STEREO.0 as f32, POS_STEREO.1 as f32);
        g.sprite(MONO_OFF, POS_MONO.0 as f32, POS_MONO.1 as f32);
    }

    // Visualiser.
    draw_vis(app, ctx, g);
    {
        let ButtonState { clicked, .. } =
            widgets.hit_button(&mut app.hot, B_VIS, HitBox::at(POS_VIS, VIS_W, VIS_H));
        if clicked {
            cmds.push(Cmd::CycleVisMode);
        }
        if m.right_pressed && m.over(POS_VIS.0, POS_VIS.1, VIS_W, VIS_H) {
            *vis_menu = m.pos.map(|p| Pos2::new(p.x * scale, p.y * scale));
        }
    }

    // Volume.
    {
        let v = app.player.volume();
        let r = widgets.hslider(&mut app.hot, S_VOL, HitBox::at(POS_VOLUME, 68, 13), 14, v);
        let frame = (r.value * (VOLUME_FRAMES - 1) as f32).round() as u32;
        g.sprite(volume_bg(frame), POS_VOLUME.0 as f32, POS_VOLUME.1 as f32);
        let tx = POS_VOLUME.0 as f32 + (r.value * VOLUME_TRAVEL as f32).round();
        g.sprite(
            if r.dragging {
                VOLUME_THUMB_DOWN
            } else {
                VOLUME_THUMB
            },
            tx,
            POS_VOLUME.1 as f32 + 1.0,
        );
        if r.dragging {
            if (r.value - v).abs() > 1e-4 {
                cmds.push(Cmd::SetVolume(r.value));
            }
            app.set_status(format!("Volume: {}%", (r.value * 100.0).round() as i32));
        }
    }
    // Balance.
    {
        let b = app.player.balance();
        let r = widgets.hslider(
            &mut app.hot,
            S_BAL,
            HitBox::at(POS_BALANCE, 38, 13),
            14,
            (b + 1.0) / 2.0,
        );
        let mut nb = r.value * 2.0 - 1.0;
        if nb.abs() < 0.08 {
            nb = 0.0; // centre detent
        }
        let frame = (nb.abs() * (VOLUME_FRAMES - 1) as f32).round() as u32;
        g.sprite(
            balance_bg(frame),
            POS_BALANCE.0 as f32,
            POS_BALANCE.1 as f32,
        );
        let tx = POS_BALANCE.0 as f32 + (((nb + 1.0) / 2.0) * BALANCE_TRAVEL as f32).round();
        g.sprite(
            if r.dragging {
                BALANCE_THUMB_DOWN
            } else {
                BALANCE_THUMB
            },
            tx,
            POS_BALANCE.1 as f32 + 1.0,
        );
        if r.dragging {
            if (nb - b).abs() > 1e-4 {
                cmds.push(Cmd::SetBalance(nb));
            }
            let pct = (nb.abs() * 100.0).round() as i32;
            let msg = if nb == 0.0 {
                "Balance: center".to_string()
            } else if nb < 0.0 {
                format!("Balance: {pct}% left")
            } else {
                format!("Balance: {pct}% right")
            };
            app.set_status(msg);
        }
    }

    // EQ / PL toggles.
    let eq_on = app.windows.get(Win::Eq).visible;
    let pl_on = app.windows.get(Win::Pl).visible;
    if widgets.button(
        &mut app.hot,
        B_EQ,
        POS_EQ_BTN,
        if eq_on { EQ_ON } else { EQ_OFF },
        if eq_on { EQ_ON_DOWN } else { EQ_OFF_DOWN },
    ) {
        cmds.push(Cmd::ToggleEq);
    }
    if widgets.button(
        &mut app.hot,
        B_PL,
        POS_PL_BTN,
        if pl_on { PL_ON } else { PL_OFF },
        if pl_on { PL_ON_DOWN } else { PL_OFF_DOWN },
    ) {
        cmds.push(Cmd::TogglePlaylist);
    }

    // Position bar.
    {
        let dur = app
            .player
            .duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let pos = app.player.position().as_secs_f64();
        let has_track =
            app.player.current_source().is_some() && dur > 0.0 && state != PlayState::Stopped;
        let seekable = app.player.info().map(|i| i.seekable).unwrap_or(false);
        g.sprite(POSBAR_BG, POS_POSBAR.0 as f32, POS_POSBAR.1 as f32);
        if has_track {
            let cur = (pos / dur).clamp(0.0, 1.0) as f32;
            let r = widgets.hslider(
                &mut app.hot,
                S_POS,
                HitBox::at(POS_POSBAR, 248, 10),
                29,
                cur,
            );
            let shown = if r.dragging && seekable { r.value } else { cur };
            let tx = POS_POSBAR.0 as f32 + (shown * POSBAR_TRAVEL as f32).round();
            g.sprite(
                if r.dragging {
                    POSBAR_THUMB_DOWN
                } else {
                    POSBAR_THUMB
                },
                tx,
                POS_POSBAR.1 as f32,
            );
            if r.dragging && seekable {
                let t = r.value as f64 * dur;
                app.set_status(format!(
                    "Seek to: {} ({}%)",
                    format_duration(Duration::from_secs_f64(t)),
                    (r.value * 100.0) as i32
                ));
                if r.released {
                    cmds.push(Cmd::SeekTo(t));
                    app.status = None;
                }
            }
        }
    }

    // Transport buttons.
    if widgets.button(&mut app.hot, B_PREV, POS_BTN_PREV, BTN_PREV, BTN_PREV_DOWN) {
        cmds.push(Cmd::Prev);
    }
    if widgets.button(&mut app.hot, B_PLAY, POS_BTN_PLAY, BTN_PLAY, BTN_PLAY_DOWN) {
        cmds.push(Cmd::Play);
    }
    if widgets.button(
        &mut app.hot,
        B_PAUSE,
        POS_BTN_PAUSE,
        BTN_PAUSE,
        BTN_PAUSE_DOWN,
    ) {
        cmds.push(Cmd::Pause);
    }
    if widgets.button(&mut app.hot, B_STOP, POS_BTN_STOP, BTN_STOP, BTN_STOP_DOWN) {
        cmds.push(Cmd::Stop);
    }
    if widgets.button(&mut app.hot, B_NEXT, POS_BTN_NEXT, BTN_NEXT, BTN_NEXT_DOWN) {
        cmds.push(Cmd::Next);
    }
    if widgets.button(
        &mut app.hot,
        B_EJECT,
        POS_BTN_EJECT,
        BTN_EJECT,
        BTN_EJECT_DOWN,
    ) {
        cmds.push(Cmd::OpenFiles);
    }

    // Shuffle / repeat.
    let (sh, rp) = (app.cfg.shuffle, app.cfg.repeat);
    if widgets.button(
        &mut app.hot,
        B_SHUFFLE,
        POS_SHUFFLE,
        if sh { SHUFFLE_ON } else { SHUFFLE_OFF },
        if sh {
            SHUFFLE_ON_DOWN
        } else {
            SHUFFLE_OFF_DOWN
        },
    ) {
        cmds.push(Cmd::ToggleShuffle);
    }
    if widgets.button(
        &mut app.hot,
        B_REPEAT,
        POS_REPEAT,
        if rp { REPEAT_ON } else { REPEAT_OFF },
        if rp { REPEAT_ON_DOWN } else { REPEAT_OFF_DOWN },
    ) {
        cmds.push(Cmd::ToggleRepeat);
    }
}

/// What the four-digit LCD shows: `-mm:ss` remaining or `mm:ss` elapsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClockDigits {
    negative: bool,
    minutes: u32,
    seconds: u32,
}

/// Elapsed or remaining time for the LCD; minutes saturate at 99.
fn clock_digits(position: Duration, duration: Option<Duration>, mode: TimeMode) -> ClockDigits {
    let (negative, shown) = match (mode, duration) {
        (TimeMode::Remaining, Some(duration)) => (true, duration.saturating_sub(position)),
        _ => (false, position),
    };
    let total_secs = shown.as_secs();
    ClockDigits {
        negative,
        minutes: (total_secs / 60).min(99) as u32,
        seconds: (total_secs % 60) as u32,
    }
}

/// The LCD digits for the current track, or None when nothing is loaded.
fn time_digits(app: &App) -> Option<ClockDigits> {
    if app.player.state() == PlayState::Stopped || app.player.current_source().is_none() {
        return None;
    }
    Some(clock_digits(
        app.player.position(),
        app.player.duration(),
        app.cfg.time_mode,
    ))
}

/// Text that scrolls past the end of the title before it wraps around.
const MARQUEE_SEPARATOR: &str = "  ***  ";
/// Characters visible in the windowshade title field.
const SHADE_MARQUEE_CHARS: usize = 19;

/// The `width` (+1 for the partially visible last cell) characters of `text`
/// visible after scrolling `offset` characters. Texts that fit are shown whole.
fn marquee_window(text: &str, offset: i32, width: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_string();
    }
    let looped: Vec<char> = chars.into_iter().chain(MARQUEE_SEPARATOR.chars()).collect();
    let start = offset.rem_euclid(looped.len() as i32) as usize;
    (0..=width)
        .map(|i| looped[(start + i) % looped.len()])
        .collect()
}

fn draw_time(app: &mut App, g: &Gfx, m: &Mouse, cmds: &mut Vec<Cmd>) {
    let widgets = Widgets::new(g, m, Win::Main);
    let blink_hidden = app.player.state() == PlayState::Paused
        && (app.started.elapsed().as_millis() / 500) % 2 == 1;
    match time_digits(app) {
        Some(ClockDigits {
            negative,
            minutes,
            seconds,
        }) if !blink_hidden => {
            g.minus(
                POS_TIME_MINUS_EX.0 as f32,
                POS_TIME_MINUS_EX.1 as f32,
                negative,
            );
            let digits = [minutes / 10, minutes % 10, seconds / 10, seconds % 10];
            for (digit, (x, y)) in digits.into_iter().zip(POS_TIME_DIGITS) {
                g.digit(x as f32, y as f32, digit);
            }
        }
        _ => {
            g.digit_blank(POS_TIME_MINUS_EX.0 as f32, POS_TIME_MINUS_EX.1 as f32);
            for (x, y) in POS_TIME_DIGITS {
                g.digit_blank(x as f32, y as f32);
            }
        }
    }
    let ButtonState { clicked, .. } =
        widgets.hit_button(&mut app.hot, B_TIME, HitBox::new(36, 26, 63, 13));
    if clicked {
        cmds.push(Cmd::ToggleTimeMode);
    }
}

fn draw_marquee(app: &mut App, g: &Gfx, m: &Mouse) {
    let text = app.marquee_text();
    let (x, y) = (POS_MARQUEE.0 as f32, POS_MARQUEE.1 as f32);
    let visible_chars = (MARQUEE_W / FONT_CELL_W) as usize;
    // Status messages are shown whole instead of scrolling.
    let is_status = app
        .status
        .as_ref()
        .is_some_and(|(_, shown_at)| shown_at.elapsed() < Duration::from_secs(3));
    let shown = if is_status {
        text
    } else {
        marquee_window(&text, app.marquee_offset, visible_chars)
    };
    g.text_clipped(x, y, &shown, x, MARQUEE_W as f32);
    // Dragging the marquee scrolls it manually.
    if m.pressed && m.over(POS_MARQUEE.0, POS_MARQUEE.1, MARQUEE_W, 6) && app.hot.is_none() {
        if let Some(p) = m.pos {
            app.hot = Some(Hot::Marquee(p.x));
        }
    }
    if let Some(Hot::Marquee(start)) = app.hot {
        if let Some(p) = m.pos {
            let dx = ((start - p.x) / 5.0) as i32;
            if dx != 0 {
                app.marquee_offset += dx;
                app.hot = Some(Hot::Marquee(start - dx as f32 * 5.0));
            }
        }
    }
}

pub fn draw_vis(app: &mut App, ctx: &egui::Context, g: &Gfx) {
    let colors = &app.skin.viscolors.0;
    let bg = rgb(colors[0]);
    let grid = rgb(colors[1]);
    let surf = &mut app.vis.surface;
    surf.clear(bg);
    for y in (0..16).step_by(2) {
        for x in (0..76).step_by(2) {
            surf.px(x, y, grid);
        }
    }
    match app.cfg.vis_mode {
        VisMode::Off => {}
        VisMode::Analyzer => {
            let (n, bw) = match app.cfg.analyzer_style {
                AnalyzerStyle::Bars => (19usize, 3i32),
                AnalyzerStyle::Lines => (75usize, 1i32),
            };
            for i in 0..n {
                let h = app.vis.bars[i].round().clamp(0.0, 16.0) as i32;
                let x0 = if bw == 3 { i as i32 * 4 } else { i as i32 };
                for yy in 0..h {
                    let y = 15 - yy;
                    let ci = (2 + (15 - yy)).clamp(2, 17) as usize;
                    let c = rgb(colors[ci]);
                    for dx in 0..bw {
                        surf.px(x0 + dx, y, c);
                    }
                }
                if app.cfg.analyzer_peaks {
                    let p = app.vis.peaks[i].round().clamp(0.0, 16.0) as i32;
                    if p > 0 {
                        let y = 16 - p;
                        let c = rgb(colors[23]);
                        for dx in 0..bw {
                            surf.px(x0 + dx, y, c);
                        }
                    }
                }
            }
        }
        VisMode::Scope => {
            let samples = winamp_audio::vis::oscilloscope(&app.vis_samples_cache, 76);
            let mut prev_y: Option<i32> = None;
            for (x, s) in samples.iter().enumerate() {
                let y = (8.0 - s * 7.0).round().clamp(0.0, 15.0) as i32;
                let zone = ((y - 8).abs() / 2).clamp(0, 4) as usize;
                let c = rgb(colors[18 + zone]);
                match prev_y {
                    Some(py) if (py - y).abs() > 1 => {
                        let (a, b) = if py < y { (py + 1, y) } else { (y, py - 1) };
                        surf.vline(x as i32, a, b, c);
                    }
                    _ => surf.px(x as i32, y, c),
                }
                prev_y = Some(y);
            }
        }
    }
    surf.draw(ctx, g, POS_VIS.0 as f32, POS_VIS.1 as f32);
}

fn draw_shade(
    app: &mut App,
    g: &Gfx,
    m: &Mouse,
    focused: bool,
    cmds: &mut Vec<Cmd>,
    open_menu: &mut Option<Pos2>,
) {
    let scale = g.scale;
    let widgets = Widgets::new(g, m, Win::Main);
    g.sprite(
        if focused {
            TITLE_SHADE_ACTIVE
        } else {
            TITLE_SHADE_INACTIVE
        },
        0.0,
        0.0,
    );
    let hot = &mut app.hot;
    if widgets.button(
        hot,
        B_OPTIONS,
        POS_BTN_OPTIONS,
        BTN_OPTIONS,
        BTN_OPTIONS_DOWN,
    ) {
        *open_menu = Some(Pos2::new(6.0 * scale, 14.0 * scale));
    }
    if widgets.button(
        hot,
        B_MINIMIZE,
        POS_BTN_MINIMIZE,
        BTN_MINIMIZE,
        BTN_MINIMIZE_DOWN,
    ) {
        cmds.push(Cmd::MinimizeAll);
    }
    if widgets.button(hot, B_SHADE, POS_BTN_SHADE, BTN_UNSHADE, BTN_UNSHADE_DOWN) {
        cmds.push(Cmd::ToggleMainShade);
    }
    if widgets.button(hot, B_CLOSE, POS_BTN_CLOSE, BTN_CLOSE, BTN_CLOSE_DOWN) {
        cmds.push(Cmd::Exit);
    }
    // Title text (scrolling) and time.
    let shown = marquee_window(&app.marquee_text(), app.marquee_offset, SHADE_MARQUEE_CHARS);
    g.text_clipped(80.0, 4.0, &shown, 80.0, 95.0);
    if let Some(ClockDigits {
        negative,
        minutes,
        seconds,
    }) = time_digits(app)
    {
        let sign = if negative { "-" } else { " " };
        g.text_clipped(
            197.0,
            4.0,
            &format!("{sign}{minutes:02}:{seconds:02}"),
            196.0,
            30.0,
        );
    }
    {
        let ButtonState { clicked, .. } =
            widgets.hit_button(&mut app.hot, B_TIME, HitBox::new(196, 3, 30, 9));
        if clicked {
            cmds.push(Cmd::ToggleTimeMode);
        }
    }
    // Mini position bar.
    let dur = app
        .player
        .duration()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    g.sprite(SHADE_POS_BG, POS_SHADE_POS.0 as f32, POS_SHADE_POS.1 as f32);
    if dur > 0.0 && app.player.state() != PlayState::Stopped {
        let cur = (app.player.position().as_secs_f64() / dur).clamp(0.0, 1.0) as f32;
        let r = widgets.hslider(
            &mut app.hot,
            S_POS,
            HitBox::at(POS_SHADE_POS, 17, 7),
            3,
            cur,
        );
        let v = if r.dragging { r.value } else { cur };
        let tx = POS_SHADE_POS.0 as f32 + (v * 14.0).round();
        let sp = if v < 0.33 {
            SHADE_POS_THUMB_LEFT
        } else if v > 0.66 {
            SHADE_POS_THUMB_RIGHT
        } else {
            SHADE_POS_THUMB
        };
        g.sprite(sp, tx, POS_SHADE_POS.1 as f32);
        if r.released {
            cmds.push(Cmd::SeekTo(r.value as f64 * dur));
        }
    }
    if m.double_clicked && m.pos.map(|p| p.y < 14.0 && p.x < 240.0).unwrap_or(false) {
        cmds.push(Cmd::ToggleMainShade);
    }
}

pub fn main_menu_items(app: &App) -> Vec<Item> {
    use winamp_playlist::SortKey;
    let eq = app.player.eq();
    let skins = skin_menu_items(app);
    vec![
        Item::action("Nullsoft Winamp...", Cmd::About),
        Item::Sep,
        Item::sub(
            "Play",
            vec![
                Item::action("File...  (L)", Cmd::OpenFiles),
                Item::action("Folder...  (Shift+L)", Cmd::OpenDirectory),
                Item::action("Location (URL)...  (Ctrl+L)", Cmd::OpenUrl),
            ],
        ),
        Item::sub("Internet Radio", radio_menu_items()),
        Item::action("View file info...  (Alt+3)", Cmd::FileInfo),
        Item::Sep,
        Item::check("Main Window  (Alt+W)", Cmd::ToggleMain, !app.main_hidden),
        Item::check(
            "Playlist Editor  (Alt+E)",
            Cmd::TogglePlaylist,
            app.windows.get(Win::Pl).visible,
        ),
        Item::check(
            "Equalizer  (Alt+G)",
            Cmd::ToggleEq,
            app.windows.get(Win::Eq).visible,
        ),
        Item::Sep,
        Item::sub(
            "Options",
            vec![
                Item::action("Preferences...  (Ctrl+P)", Cmd::Preferences),
                Item::sub("Skins", skins),
                Item::Sep,
                Item::check(
                    "Time elapsed",
                    Cmd::ToggleTimeMode,
                    app.cfg.time_mode == TimeMode::Elapsed,
                ),
                Item::check(
                    "Time remaining",
                    Cmd::ToggleTimeMode,
                    app.cfg.time_mode == TimeMode::Remaining,
                ),
                Item::check(
                    "Always On Top  (Ctrl+A)",
                    Cmd::ToggleAlwaysOnTop,
                    app.cfg.always_on_top,
                ),
                Item::check(
                    "Double Size  (Ctrl+D)",
                    Cmd::ToggleDoubleSize,
                    app.cfg.double_size,
                ),
                Item::check(
                    "Lock windows together",
                    Cmd::ToggleLockWindows,
                    app.cfg.lock_windows,
                ),
                Item::check("Scroll title", Cmd::ToggleScrollTitle, app.cfg.scroll_title),
                Item::Sep,
                Item::check("Repeat  (R)", Cmd::ToggleRepeat, app.cfg.repeat),
                Item::check("Shuffle  (S)", Cmd::ToggleShuffle, app.cfg.shuffle),
                Item::Sep,
                Item::check(
                    "Windowshade mode  (Ctrl+W)",
                    Cmd::ToggleMainShade,
                    app.windows.get(Win::Main).shaded,
                ),
                Item::check(
                    "EQ windowshade",
                    Cmd::ToggleEqShade,
                    app.windows.get(Win::Eq).shaded,
                ),
                Item::check(
                    "Playlist windowshade",
                    Cmd::TogglePlShade,
                    app.windows.get(Win::Pl).shaded,
                ),
            ],
        ),
        Item::sub(
            "Playback",
            vec![
                Item::action("Previous  (Z)", Cmd::Prev),
                Item::action("Play  (X)", Cmd::Play),
                Item::action("Pause  (C)", Cmd::Pause),
                Item::action("Stop  (V)", Cmd::Stop),
                Item::action("Next  (B)", Cmd::Next),
                Item::Sep,
                Item::action("Back 5 seconds  (Left)", Cmd::SeekRelative(-5.0)),
                Item::action("Fwd 5 seconds  (Right)", Cmd::SeekRelative(5.0)),
                Item::action("Jump to file...  (J)", Cmd::JumpToFile),
                Item::action("Jump to time...  (Ctrl+J)", Cmd::JumpToTime),
                Item::Sep,
                Item::check("EQ enabled", Cmd::ToggleEqEnabled, eq.enabled),
                Item::action("EQ presets...", Cmd::EqPresetsDialog),
            ],
        ),
        Item::sub("Visualization", vis_menu_items(app)),
        Item::sub(
            "Playlist",
            vec![
                Item::action(
                    crate::app::sort_key_label(SortKey::Title),
                    Cmd::PlSort(SortKey::Title),
                ),
                Item::action(
                    crate::app::sort_key_label(SortKey::Filename),
                    Cmd::PlSort(SortKey::Filename),
                ),
                Item::action(
                    crate::app::sort_key_label(SortKey::Path),
                    Cmd::PlSort(SortKey::Path),
                ),
                Item::action("Reverse list", Cmd::PlReverse),
                Item::action("Randomize list", Cmd::PlRandomize),
                Item::Sep,
                Item::action("Open playlist...", Cmd::PlLoad),
                Item::action("Save playlist...", Cmd::PlSave),
                Item::action("Clear playlist", Cmd::PlRemoveAll),
            ],
        ),
        Item::Sep,
        Item::action("Exit", Cmd::Exit),
    ]
}

pub fn radio_menu_items() -> Vec<Item> {
    let mut items = vec![
        Item::action("Open location (URL)...  (Ctrl+L)", Cmd::OpenUrl),
        Item::Sep,
    ];
    for g in crate::radio::GROUPS {
        let stations: Vec<Item> = g
            .stations
            .iter()
            .map(|st| Item::action(st.name, Cmd::PlayUrl(st.url.to_string())))
            .collect();
        items.push(Item::sub(g.name, stations));
    }
    items
}

pub fn vis_menu_items(app: &App) -> Vec<Item> {
    vec![
        Item::check(
            "Spectrum Analyzer",
            Cmd::SetVisMode(VisMode::Analyzer),
            app.cfg.vis_mode == VisMode::Analyzer,
        ),
        Item::check(
            "Oscilloscope",
            Cmd::SetVisMode(VisMode::Scope),
            app.cfg.vis_mode == VisMode::Scope,
        ),
        Item::check(
            "Off",
            Cmd::SetVisMode(VisMode::Off),
            app.cfg.vis_mode == VisMode::Off,
        ),
        Item::Sep,
        Item::check(
            "Analyzer: Bars",
            Cmd::ToggleAnalyzerStyle,
            app.cfg.analyzer_style == AnalyzerStyle::Bars,
        ),
        Item::check(
            "Analyzer: Lines",
            Cmd::ToggleAnalyzerStyle,
            app.cfg.analyzer_style == AnalyzerStyle::Lines,
        ),
        Item::check(
            "Analyzer peaks",
            Cmd::ToggleAnalyzerPeaks,
            app.cfg.analyzer_peaks,
        ),
        Item::Sep,
        Item::check(
            "Milkdrop visualizer  (Ctrl+Shift+K)",
            Cmd::ToggleMilkdrop,
            app.milk.is_some(),
        ),
    ]
}

pub fn skin_menu_items(app: &App) -> Vec<Item> {
    let mut items = vec![
        Item::check("<Base Skin>", Cmd::LoadSkin(None), app.cfg.skin.is_none()),
        Item::action("Skin browser...  (Alt+S)", Cmd::SkinBrowser),
        Item::Sep,
    ];
    for p in crate::dialogs::list_skins() {
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let checked = app.cfg.skin.as_ref() == Some(&p);
        items.push(Item::check(&name, Cmd::LoadSkin(Some(p)), checked));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_titles_are_not_scrolled() {
        assert_eq!(marquee_window("Short", 7, 30), "Short");
        assert_eq!(marquee_window("", 3, 30), "");
    }

    #[test]
    fn long_titles_scroll_and_wrap_with_a_separator() {
        let title = "abcdefghij"; // 10 chars, window of 4 shows 5 cells
        assert_eq!(marquee_window(title, 0, 4), "abcde");
        assert_eq!(marquee_window(title, 2, 4), "cdefg");
        assert_eq!(marquee_window(title, 10, 4), "  ***");
        let period = (title.len() + MARQUEE_SEPARATOR.len()) as i32;
        assert_eq!(
            marquee_window(title, period, 4),
            "abcde",
            "wraps after title + separator"
        );
        assert_eq!(
            marquee_window(title, -1, 4),
            " abcd",
            "negative offsets scroll backwards"
        );
    }

    #[test]
    fn clock_digits_elapsed_and_remaining() {
        let position = Duration::from_secs(65);
        let duration = Some(Duration::from_secs(200));
        assert_eq!(
            clock_digits(position, duration, TimeMode::Elapsed),
            ClockDigits {
                negative: false,
                minutes: 1,
                seconds: 5
            }
        );
        assert_eq!(
            clock_digits(position, duration, TimeMode::Remaining),
            ClockDigits {
                negative: true,
                minutes: 2,
                seconds: 15
            }
        );
        assert_eq!(
            clock_digits(position, None, TimeMode::Remaining),
            ClockDigits {
                negative: false,
                minutes: 1,
                seconds: 5
            },
            "streams have no remaining time"
        );
        assert_eq!(
            clock_digits(
                Duration::from_secs(300),
                Some(Duration::from_secs(100)),
                TimeMode::Remaining
            )
            .minutes,
            0,
            "never negative past the end"
        );
        assert_eq!(
            clock_digits(Duration::from_secs(99 * 3600), None, TimeMode::Elapsed).minutes,
            99,
            "minutes saturate"
        );
    }
}
