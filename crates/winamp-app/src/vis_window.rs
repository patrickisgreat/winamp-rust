//! The Milkdrop visualiser window.

use std::time::{Duration, Instant};

use egui::{
    Color32, ColorImage, Pos2, Rect, TextureOptions, Vec2, ViewportBuilder, ViewportCommand,
};

use crate::actions::Cmd;
use crate::app::App;
use crate::main_window::open_menu_at;
use crate::menu::Item;
use crate::milkdrop::Milkdrop;
use crate::windows::Win;

pub const MILK_W: usize = 320;
pub const MILK_H: usize = 240;

pub struct MilkState {
    pub engine: Milkdrop,
    pub tex: Option<egui::TextureHandle>,
    pub last: Instant,
    pub message: Option<(String, Instant)>,
    pub fullscreen: bool,
    rgba: Vec<u8>,
    bars: Vec<f32>,
}

impl MilkState {
    pub fn new() -> Self {
        Self {
            engine: Milkdrop::new(MILK_W, MILK_H),
            tex: None,
            last: Instant::now(),
            message: None,
            fullscreen: false,
            rgba: Vec::new(),
            bars: Vec::new(),
        }
    }

    pub fn announce(&mut self) {
        self.message = Some((self.engine.preset_name().to_string(), Instant::now()));
    }

    /// Advance the engine using the player's latest audio.
    pub fn tick(&mut self, app_samples: &[f32], out_rate: u32) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last)
            .as_secs_f32()
            .clamp(0.001, 0.1);
        self.last = now;
        self.bars = winamp_audio::vis::spectrum(app_samples, out_rate, 32);
        let bars = std::mem::take(&mut self.bars);
        self.engine.step(dt, app_samples, &bars);
        self.bars = bars;
    }

    pub fn upload(&mut self, ctx: &egui::Context) {
        self.engine.rgba(&mut self.rgba);
        let img = ColorImage::from_rgba_unmultiplied([self.engine.w, self.engine.h], &self.rgba);
        match &mut self.tex {
            Some(t) => t.set(img, TextureOptions::LINEAR),
            None => self.tex = Some(ctx.load_texture("milkdrop", img, TextureOptions::LINEAR)),
        }
    }
}

pub fn builder(app: &App) -> ViewportBuilder {
    let mut b = ViewportBuilder::default()
        .with_title("Winamp Milkdrop")
        .with_inner_size(Vec2::new(640.0, 480.0))
        .with_min_inner_size(Vec2::new(160.0, 120.0))
        .with_resizable(true)
        .with_window_level(app.window_level());
    if let Some(m) = &app.milk {
        if m.fullscreen {
            b = b.with_fullscreen(true);
        }
    }
    b
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut cmds: Vec<Cmd> = Vec::new();
    if ctx.input(|i| i.viewport().close_requested()) {
        cmds.push(Cmd::ToggleMilkdrop);
    }
    let samples = app.player.vis_samples(1024);
    let rate = app.player.output_sample_rate();
    let playing = app.player.state() == winamp_audio::PlayState::Playing;
    let silent = vec![0.0f32; 1024];
    let menu_open = app.menu.is_some();
    if let Some(m) = &mut app.milk {
        m.tick(if playing { &samples } else { &silent }, rate);
        m.upload(ctx);
    }
    let mut open_menu: Option<Pos2> = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(Color32::BLACK))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            if let Some(m) = &app.milk {
                if let Some(t) = &m.tex {
                    ui.painter().image(
                        t.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if let Some((msg, at)) = &m.message {
                    if at.elapsed() < Duration::from_secs(3) {
                        ui.painter().text(
                            rect.min + Vec2::new(12.0, 10.0),
                            egui::Align2::LEFT_TOP,
                            msg,
                            egui::FontId::proportional(16.0),
                            Color32::from_white_alpha(220),
                        );
                    }
                }
            }
            let r = ui.interact(rect, ui.id().with("milk"), egui::Sense::click());
            if r.double_clicked() {
                cmds.push(Cmd::MilkdropFullscreen);
            }
            if r.secondary_clicked() && !menu_open {
                open_menu = r.interact_pointer_pos();
            }
            ctx.input(|i| {
                use egui::Key;
                for ev in &i.events {
                    if let egui::Event::Key {
                        key, pressed: true, ..
                    } = ev
                    {
                        match key {
                            Key::Space | Key::N | Key::ArrowRight => {
                                cmds.push(Cmd::MilkdropPreset(1))
                            }
                            Key::P | Key::Backspace | Key::ArrowLeft => {
                                cmds.push(Cmd::MilkdropPreset(-1))
                            }
                            Key::R => cmds.push(Cmd::MilkdropPreset(0)),
                            Key::F | Key::Enter => cmds.push(Cmd::MilkdropFullscreen),
                            Key::Escape => {
                                if app.milk.as_ref().map(|m| m.fullscreen).unwrap_or(false) {
                                    cmds.push(Cmd::MilkdropFullscreen);
                                } else {
                                    cmds.push(Cmd::ToggleMilkdrop);
                                }
                            }
                            Key::X => cmds.push(Cmd::Play),
                            Key::C => cmds.push(Cmd::Pause),
                            Key::V => cmds.push(Cmd::Stop),
                            Key::B => cmds.push(Cmd::Next),
                            Key::Z => cmds.push(Cmd::Prev),
                            _ => {}
                        }
                    }
                }
            });
        });
    if let Some(pos) = open_menu {
        let items = milkdrop_menu_items(app);
        let origin = ctx
            .input(|i| i.viewport().outer_rect.map(|r| r.min))
            .unwrap_or(Pos2::ZERO);
        app.menu = Some((
            Win::Main,
            crate::menu::Menu::new(items, origin + pos.to_vec2()),
        ));
    }
    for c in cmds {
        app.exec(c, ctx);
    }
    let _ = open_menu_at;
}

pub fn milkdrop_menu_items(app: &App) -> Vec<Item> {
    let mut items = vec![
        Item::action("Next preset  (Space)", Cmd::MilkdropPreset(1)),
        Item::action("Previous preset  (Backspace)", Cmd::MilkdropPreset(-1)),
        Item::action("Random preset  (R)", Cmd::MilkdropPreset(0)),
        Item::action("Fullscreen  (F)", Cmd::MilkdropFullscreen),
        Item::Sep,
    ];
    if let Some(m) = &app.milk {
        for (i, p) in m.engine.presets.iter().enumerate() {
            items.push(Item::check(
                p.name,
                Cmd::MilkdropSetPreset(i),
                i == m.engine.preset,
            ));
        }
    }
    items.push(Item::Sep);
    items.push(Item::action("Close Milkdrop  (Esc)", Cmd::ToggleMilkdrop));
    items
}

pub fn send_fullscreen(ctx: &egui::Context, on: bool) {
    ctx.send_viewport_cmd_to(milk_viewport_id(), ViewportCommand::Fullscreen(on));
}

pub fn milk_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("winamp-milkdrop")
}
