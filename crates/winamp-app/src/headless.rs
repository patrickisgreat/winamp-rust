//! Headless software rendering of the skinned windows, used for
//! `--screenshot DIR` (visual regression / debugging without a display).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use egui::epaint::{ClippedPrimitive, ImageData, Primitive, TextureId};
use egui::{Color32, Pos2, RawInput, Rect, Vec2};

use crate::app::App;
use crate::windows::Win;

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

#[derive(Default)]
struct TextureStore {
    map: HashMap<TextureId, Texture>,
}

impl TextureStore {
    fn apply(&mut self, delta: &egui::TexturesDelta) {
        for (id, d) in &delta.set {
            let (w, h, px): (usize, usize, Vec<Color32>) = match &d.image {
                ImageData::Color(img) => (img.width(), img.height(), img.pixels.clone()),
                ImageData::Font(f) => (f.width(), f.height(), f.srgba_pixels(None).collect()),
            };
            match d.pos {
                None => {
                    self.map.insert(*id, Texture { w, h, px });
                }
                Some([x0, y0]) => {
                    if let Some(t) = self.map.get_mut(id) {
                        for y in 0..h {
                            for x in 0..w {
                                let (tx, ty) = (x0 + x, y0 + y);
                                if tx < t.w && ty < t.h {
                                    t.px[ty * t.w + tx] = px[y * w + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.map.remove(id);
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> Color32 {
        match self.map.get(&id) {
            Some(t) => {
                let x = ((u * t.w as f32).floor() as isize).clamp(0, t.w as isize - 1) as usize;
                let y = ((v * t.h as f32).floor() as isize).clamp(0, t.h as isize - 1) as usize;
                t.px[y * t.w + x]
            }
            None => Color32::WHITE,
        }
    }
}

struct Frame {
    w: usize,
    px: Vec<[f32; 4]>, // premultiplied rgba 0..1
}

impl Frame {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            px: vec![[0.0, 0.0, 0.0, 1.0]; w * h],
        }
    }

    fn blend(&mut self, x: usize, y: usize, c: [f32; 4]) {
        let d = &mut self.px[y * self.w + x];
        let a = c[3];
        for i in 0..4 {
            d[i] = c[i] + d[i] * (1.0 - a);
        }
    }

    fn to_rgba(&self) -> Vec<u8> {
        self.px
            .iter()
            .flat_map(|p| {
                p.iter()
                    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
                    .collect::<Vec<u8>>()
            })
            .collect()
    }
}

/// Straight RGBA in 0..=1.
fn color_to_f32(c: Color32) -> [f32; 4] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    ]
}

fn rasterize(prims: &[ClippedPrimitive], store: &TextureStore, w: usize, h: usize) -> Frame {
    let mut frame = Frame::new(w, h);
    for p in prims {
        let Primitive::Mesh(mesh) = &p.primitive else {
            continue;
        };
        let clip = p.clip_rect;
        for tri in mesh.indices.chunks_exact(3) {
            let v0 = &mesh.vertices[tri[0] as usize];
            let v1 = &mesh.vertices[tri[1] as usize];
            let v2 = &mesh.vertices[tri[2] as usize];
            let min_x = v0
                .pos
                .x
                .min(v1.pos.x)
                .min(v2.pos.x)
                .max(clip.min.x)
                .floor()
                .max(0.0) as usize;
            let max_x = v0
                .pos
                .x
                .max(v1.pos.x)
                .max(v2.pos.x)
                .min(clip.max.x)
                .ceil()
                .min(w as f32) as usize;
            let min_y = v0
                .pos
                .y
                .min(v1.pos.y)
                .min(v2.pos.y)
                .max(clip.min.y)
                .floor()
                .max(0.0) as usize;
            let max_y = v0
                .pos
                .y
                .max(v1.pos.y)
                .max(v2.pos.y)
                .min(clip.max.y)
                .ceil()
                .min(h as f32) as usize;
            let area = (v1.pos.x - v0.pos.x) * (v2.pos.y - v0.pos.y)
                - (v2.pos.x - v0.pos.x) * (v1.pos.y - v0.pos.y);
            if area.abs() < 1e-6 {
                continue;
            }
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let w0 = ((v1.pos.x - px) * (v2.pos.y - py)
                        - (v2.pos.x - px) * (v1.pos.y - py))
                        / area;
                    let w1 = ((v2.pos.x - px) * (v0.pos.y - py)
                        - (v0.pos.x - px) * (v2.pos.y - py))
                        / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                        continue;
                    }
                    let u = v0.uv.x * w0 + v1.uv.x * w1 + v2.uv.x * w2;
                    let v = v0.uv.y * w0 + v1.uv.y * w1 + v2.uv.y * w2;
                    let c0 = color_to_f32(v0.color);
                    let c1 = color_to_f32(v1.color);
                    let c2 = color_to_f32(v2.color);
                    let mut c = [0.0f32; 4];
                    for i in 0..4 {
                        c[i] = c0[i] * w0 + c1[i] * w1 + c2[i] * w2;
                    }
                    let t = color_to_f32(store.sample(mesh.texture_id, u, v));
                    for i in 0..4 {
                        c[i] *= t[i];
                    }
                    if c[3] <= 0.001 {
                        continue;
                    }
                    frame.blend(x, y, c);
                }
            }
        }
    }
    frame
}

/// Render every window to `dir/<name>.png` after loading `args` into the playlist.
pub fn run(args: Vec<String>, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let ctx = egui::Context::default();
    ctx.set_visuals(egui::Visuals::dark());
    let mut app = App::new(&ctx, args)?;
    // Let playback get going so the display shows something.
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let mut store = TextureStore::default();
    let scale = app.scale();
    let shots: Vec<(&str, Win, Vec2)> = vec![
        ("main", Win::Main, Vec2::new(275.0, 116.0)),
        ("eq", Win::Eq, Vec2::new(275.0, 116.0)),
        ("pl", Win::Pl, app.pl_size()),
    ];
    for (name, win, size) in shots {
        let size = size * scale;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            ..Default::default()
        };
        // A few warm-up frames let textures upload and layout settle; keep the last one.
        let mut render_frame = |app: &mut App| {
            app.poll(&ctx);
            app.update_vis();
            app.tick_marquee();
            let full = ctx.run(raw.clone(), |ctx| match win {
                Win::Main => crate::main_window::show(app, ctx),
                Win::Eq => crate::eq_window::show(app, ctx),
                Win::Pl => crate::pl_window::show(app, ctx),
            });
            store.apply(&full.textures_delta);
            full
        };
        let mut full = render_frame(&mut app);
        for _ in 0..2 {
            full = render_frame(&mut app);
        }
        let prims = ctx.tessellate(full.shapes, full.pixels_per_point);
        let (w, h) = (size.x as usize, size.y as usize);
        let frame = rasterize(&prims, &store, w, h);
        save_png(
            &dir.join(format!("{name}.png")),
            w as u32,
            h as u32,
            frame.to_rgba(),
        )?;
    }
    // Milkdrop: run the engine on a synthetic signal and dump a frame.
    let mut milk = crate::milkdrop::Milkdrop::new(320, 240);
    let wave: Vec<f32> = (0..1024)
        .map(|i| ((i as f32) * 0.07).sin() * 0.8 + ((i as f32) * 0.23).sin() * 0.3)
        .collect();
    let spec: Vec<f32> = (0..32).map(|i| (1.0 - i as f32 / 32.0) * 0.8).collect();
    for preset in 0..milk.presets.len() {
        milk.set_preset(preset);
        for _ in 0..90 {
            milk.step(1.0 / 30.0, &wave, &spec);
        }
        let mut rgba = Vec::new();
        milk.rgba(&mut rgba);
        save_png(&dir.join(format!("milkdrop-{preset}.png")), 320, 240, rgba)?;
    }
    app.player.stop();
    Ok(())
}

fn save_png(path: &Path, width: u32, height: u32, rgba: Vec<u8>) -> Result<()> {
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| anyhow!("pixel buffer does not match {width}x{height}"))?;
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Scripted self-test: drives the skinned windows with synthetic pointer and
// keyboard events and checks the resulting application state.
// ---------------------------------------------------------------------------

struct Driver {
    ctx: egui::Context,
    app: App,
    time: f64,
    failures: Vec<String>,
}

impl Driver {
    fn frame(&mut self, win: Win, events: Vec<egui::Event>) {
        self.frame_mod(win, events, egui::Modifiers::NONE);
        // Run the other visible windows too, as the real app does every frame.
        for other in [Win::Main, Win::Eq, Win::Pl] {
            if other != win && self.app.windows.get(other).visible {
                self.frame_only(other);
            }
        }
    }

    fn frame_only(&mut self, win: Win) {
        let size = match win {
            Win::Main => Vec2::new(
                275.0,
                if self.app.windows.get(Win::Main).shaded {
                    14.0
                } else {
                    116.0
                },
            ),
            Win::Eq => Vec2::new(275.0, 116.0),
            Win::Pl => self.app.pl_size(),
        } * self.app.scale();
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(self.time),
            ..Default::default()
        };
        let app = &mut self.app;
        let _ = self.ctx.run(raw, |ctx| match win {
            Win::Main => crate::main_window::show(app, ctx),
            Win::Eq => crate::eq_window::show(app, ctx),
            Win::Pl => crate::pl_window::show(app, ctx),
        });
    }

    fn settle(&mut self, win: Win) {
        std::thread::sleep(std::time::Duration::from_millis(80));
        self.idle(win, 1);
    }

    fn frame_mod(&mut self, win: Win, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        let size = match win {
            Win::Main => Vec2::new(
                275.0,
                if self.app.windows.get(Win::Main).shaded {
                    14.0
                } else {
                    116.0
                },
            ),
            Win::Eq => Vec2::new(275.0, 116.0),
            Win::Pl => self.app.pl_size(),
        } * self.app.scale();
        self.time += 1.0 / 30.0;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(self.time),
            events,
            modifiers,
            ..Default::default()
        };
        self.app.poll(&self.ctx);
        self.app.update_vis();
        self.app.tick_marquee();
        let app = &mut self.app;
        let _ = self.ctx.run(raw, |ctx| {
            match win {
                Win::Main => crate::main_window::show(app, ctx),
                Win::Eq => crate::eq_window::show(app, ctx),
                Win::Pl => crate::pl_window::show(app, ctx),
            }
            crate::hotkeys::handle(app, ctx, win);
        });
    }

    fn idle(&mut self, win: Win, n: usize) {
        for _ in 0..n {
            self.frame(win, vec![]);
        }
    }

    fn click_at(&mut self, win: Win, x: f32, y: f32, button: egui::PointerButton) {
        let pos = Pos2::new(x, y);
        self.frame(win, vec![egui::Event::PointerMoved(pos)]);
        self.frame(
            win,
            vec![egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Default::default(),
            }],
        );
        self.frame(
            win,
            vec![egui::Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        self.frame(win, vec![]);
    }

    fn click(&mut self, win: Win, x: f32, y: f32) {
        self.click_at(win, x, y, egui::PointerButton::Primary);
    }

    fn click_mod(&mut self, win: Win, x: f32, y: f32, modifiers: egui::Modifiers) {
        let pos = Pos2::new(x, y);
        let button = egui::PointerButton::Primary;
        self.frame_mod(win, vec![egui::Event::PointerMoved(pos)], modifiers);
        self.frame_mod(
            win,
            vec![egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers,
            }],
            modifiers,
        );
        self.frame_mod(
            win,
            vec![egui::Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers,
            }],
            modifiers,
        );
        self.frame(win, vec![]);
    }

    fn double_click(&mut self, win: Win, x: f32, y: f32) {
        let pos = Pos2::new(x, y);
        let button = egui::PointerButton::Primary;
        self.frame(win, vec![egui::Event::PointerMoved(pos)]);
        for _ in 0..2 {
            self.frame(
                win,
                vec![egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers: Default::default(),
                }],
            );
            self.frame(
                win,
                vec![egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
        }
        self.frame(win, vec![]);
    }

    fn drag(&mut self, win: Win, from: (f32, f32), to: (f32, f32)) {
        let a = Pos2::new(from.0, from.1);
        let b = Pos2::new(to.0, to.1);
        let button = egui::PointerButton::Primary;
        self.frame(win, vec![egui::Event::PointerMoved(a)]);
        self.frame(
            win,
            vec![egui::Event::PointerButton {
                pos: a,
                button,
                pressed: true,
                modifiers: Default::default(),
            }],
        );
        let mid = Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        self.frame(win, vec![egui::Event::PointerMoved(mid)]);
        self.frame(win, vec![egui::Event::PointerMoved(b)]);
        self.frame(
            win,
            vec![egui::Event::PointerButton {
                pos: b,
                button,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        self.frame(win, vec![]);
    }

    fn key(&mut self, win: Win, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame_mod(
            win,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            modifiers,
        );
        self.frame_mod(
            win,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            }],
            modifiers,
        );
        self.frame(win, vec![]);
    }

    fn check(&mut self, name: &str, ok: bool) {
        if ok {
            println!("ok   - {name}");
        } else {
            println!("FAIL - {name}");
            self.failures.push(name.to_string());
        }
    }
}

/// Drive the UI through a scripted session and check the resulting state.
/// Fails with a list of the checks that did not hold.
pub fn selftest(args: Vec<String>) -> Result<()> {
    use egui::Key;
    use egui::Modifiers;
    use winamp_audio::PlayState;

    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, Vec::new())?;
    // Start from a clean playlist and default geometry regardless of what the GUI last saved.
    app.playlist.clear();
    app.cfg.pl_extra_cols = 0;
    app.cfg.pl_extra_rows = 0;
    app.cfg.double_size = false;
    app.cfg.shuffle = false;
    app.cfg.repeat = false;
    app.cfg.always_on_top = false;
    app.cfg.time_mode = crate::config::TimeMode::Elapsed;
    app.cfg.vis_mode = crate::config::VisMode::Analyzer;
    app.windows.get_mut(Win::Main).shaded = false;
    app.windows.get_mut(Win::Eq).shaded = false;
    app.windows.get_mut(Win::Pl).shaded = false;
    app.player.set_eq(winamp_audio::EqSettings::default());
    app.eq_auto = false;
    for a in &args {
        app.add_path(std::path::Path::new(a));
    }
    app.play_index(0);
    let mut d = Driver {
        ctx,
        app,
        time: 0.0,
        failures: Vec::new(),
    };
    d.app.windows.get_mut(Win::Eq).visible = true;
    d.app.windows.get_mut(Win::Pl).visible = true;
    std::thread::sleep(std::time::Duration::from_millis(600));
    d.idle(Win::Main, 3);
    let n = d.app.playlist.len();
    d.check("playlist loaded from args", n >= 3);
    d.check(
        "playing first track",
        d.app.player.state() == PlayState::Playing && d.app.playlist.current == Some(0),
    );

    // Transport buttons.
    d.click(Win::Main, 73.0, 97.0); // pause
    d.settle(Win::Main);
    d.check(
        "pause button pauses",
        d.app.player.state() == PlayState::Paused,
    );
    d.click(Win::Main, 73.0, 97.0);
    d.settle(Win::Main);
    d.check(
        "pause button resumes",
        d.app.player.state() == PlayState::Playing,
    );
    d.click(Win::Main, 96.0, 97.0); // stop
    d.settle(Win::Main);
    d.check(
        "stop button stops",
        d.app.player.state() == PlayState::Stopped,
    );
    d.click(Win::Main, 50.0, 97.0); // play
    std::thread::sleep(std::time::Duration::from_millis(200));
    d.idle(Win::Main, 2);
    d.check(
        "play button plays",
        d.app.player.state() == PlayState::Playing,
    );
    d.click(Win::Main, 119.0, 97.0); // next
    d.check("next advances", d.app.playlist.current == Some(1));
    d.click(Win::Main, 27.0, 97.0); // prev
    d.check("prev goes back", d.app.playlist.current == Some(0));

    // Toggles.
    d.click(Win::Main, 187.0, 96.0);
    d.check("shuffle toggles on", d.app.cfg.shuffle);
    d.click(Win::Main, 187.0, 96.0);
    d.check("shuffle toggles off", !d.app.cfg.shuffle);
    d.click(Win::Main, 224.0, 96.0);
    d.check("repeat toggles on", d.app.cfg.repeat);
    d.click(Win::Main, 230.0, 64.0);
    d.check(
        "EQ button hides equalizer",
        !d.app.windows.get(Win::Eq).visible,
    );
    d.click(Win::Main, 230.0, 64.0);
    d.check(
        "EQ button shows equalizer",
        d.app.windows.get(Win::Eq).visible,
    );
    d.click(Win::Main, 253.0, 64.0);
    d.check(
        "PL button hides playlist",
        !d.app.windows.get(Win::Pl).visible,
    );
    d.click(Win::Main, 253.0, 64.0);
    d.check(
        "PL button shows playlist",
        d.app.windows.get(Win::Pl).visible,
    );
    d.click(Win::Main, 60.0, 32.0);
    d.check(
        "time display toggles remaining",
        d.app.cfg.time_mode == crate::config::TimeMode::Remaining,
    );
    d.click(Win::Main, 60.0, 50.0);
    d.check(
        "vis click cycles to scope",
        d.app.cfg.vis_mode == crate::config::VisMode::Scope,
    );

    // Sliders.
    d.drag(Win::Main, (112.0, 63.0), (174.0, 63.0));
    d.check("volume drag to max", d.app.player.volume() > 0.98);
    d.drag(Win::Main, (170.0, 63.0), (127.0, 63.0));
    d.check(
        "volume drag to ~25%",
        (d.app.player.volume() - 0.25).abs() < 0.08,
    );
    d.drag(Win::Main, (196.0, 63.0), (214.0, 63.0));
    d.check("balance drag right", d.app.player.balance() > 0.5);
    d.drag(Win::Main, (214.0, 63.0), (196.0, 63.0));
    d.check(
        "balance drag back to centre",
        d.app.player.balance().abs() < 0.1,
    );
    d.settle(Win::Main);
    d.settle(Win::Main);
    let dur = d
        .app
        .player
        .duration()
        .map(|x| x.as_secs_f64())
        .unwrap_or(0.0);
    d.drag(Win::Main, (30.0, 77.0), (16.0 + 110.0 + 14.0, 77.0));
    std::thread::sleep(std::time::Duration::from_millis(250));
    d.idle(Win::Main, 2);
    let pos = d.app.player.position().as_secs_f64();
    println!("     (posbar: pos={pos:.2} dur={dur:.2})");
    d.check(
        "posbar drag seeks to ~50%",
        dur > 0.0 && (pos / dur - 0.5).abs() < 0.12,
    );

    // Clutter bar & menus.
    d.click(Win::Main, 14.0, 33.0); // A = always on top
    d.check("clutter A toggles always on top", d.app.cfg.always_on_top);
    d.click(Win::Main, 14.0, 33.0);
    d.click(Win::Main, 14.0, 48.0); // D = double size
    d.check("clutter D toggles double size", d.app.cfg.double_size);
    d.click(Win::Main, 14.0, 48.0); // unscaled position must NOT hit the D button any more
    d.check(
        "double size scales hit testing (1x pos misses)",
        d.app.cfg.double_size,
    );
    d.click(Win::Main, 14.0 * 2.0, 48.0 * 2.0);
    d.check(
        "clutter D toggles back (2x pos hits)",
        !d.app.cfg.double_size,
    );
    d.click_at(Win::Main, 150.0, 100.0, egui::PointerButton::Secondary);
    d.check("right click opens menu", d.app.menu.is_some());
    d.app.menu = None;
    d.click(Win::Main, 10.0, 7.0); // options button
    d.check("options button opens menu", d.app.menu.is_some());
    d.app.menu = None;

    // Keyboard.
    d.key(Win::Main, Key::C, Modifiers::NONE);
    d.settle(Win::Main);
    d.check("C key pauses", d.app.player.state() == PlayState::Paused);
    d.key(Win::Main, Key::X, Modifiers::NONE);
    d.settle(Win::Main);
    d.check("X key plays", d.app.player.state() == PlayState::Playing);
    d.key(Win::Main, Key::W, Modifiers::COMMAND);
    d.check(
        "Ctrl+W shades main window",
        d.app.windows.get(Win::Main).shaded,
    );
    d.idle(Win::Main, 2);
    d.key(Win::Main, Key::W, Modifiers::COMMAND);
    d.check(
        "Ctrl+W unshades main window",
        !d.app.windows.get(Win::Main).shaded,
    );
    d.key(Win::Main, Key::R, Modifiers::NONE);
    d.check("R toggles repeat off", !d.app.cfg.repeat);
    d.key(Win::Main, Key::J, Modifiers::NONE);
    d.check("J opens jump dialog", d.app.dialogs.jump.is_some());
    d.app.dialogs.jump = None;
    let v0 = d.app.player.volume();
    d.key(Win::Main, Key::ArrowUp, Modifiers::NONE);
    d.check("Up raises volume", d.app.player.volume() > v0 + 0.01);

    // Equalizer window.
    d.idle(Win::Eq, 2);
    d.click(Win::Eq, 27.0, 24.0); // ON
    d.check("EQ ON button enables eq", d.app.player.eq().enabled);
    let (bx, by) = winamp_skin::sprites::pos_eq_band(0);
    d.drag(
        Win::Eq,
        (bx as f32 + 7.0, by as f32 + 31.0),
        (bx as f32 + 7.0, by as f32 + 2.0),
    );
    d.check(
        "band 0 drag to top = +12 dB",
        d.app.player.eq().bands_db[0] > 11.0,
    );
    d.drag(
        Win::Eq,
        (bx as f32 + 7.0, by as f32 + 6.0),
        (bx as f32 + 7.0, by as f32 + 62.0),
    );
    d.check(
        "band 0 drag to bottom = -12 dB",
        d.app.player.eq().bands_db[0] < -11.0,
    );
    d.drag(Win::Eq, (28.0, 95.0), (28.0, 40.0));
    d.check("preamp drag up", d.app.player.eq().preamp_db > 8.0);
    d.click(Win::Eq, 239.0, 24.0); // presets
    d.check("presets button opens dialog", d.app.dialogs.eq_presets_open);
    d.app.dialogs.eq_presets_open = false;
    d.click(Win::Eq, 55.0, 24.0); // AUTO
    d.check("AUTO toggles", d.app.eq_auto);
    d.click(Win::Eq, 268.0, 7.0); // close
    d.check("EQ close hides window", !d.app.windows.get(Win::Eq).visible);
    d.app.windows.get_mut(Win::Eq).visible = true;

    // Playlist window.
    d.idle(Win::Pl, 2);
    let row_h = d.app.pl_row_h();
    d.click(Win::Pl, 60.0, 20.0 + row_h * 1.5);
    d.check(
        "click selects row 2",
        d.app.playlist.entries[1].selected && !d.app.playlist.entries[0].selected,
    );
    d.click_mod(Win::Pl, 60.0, 20.0 + row_h * 2.5, Modifiers::SHIFT);
    d.check(
        "shift-click extends selection",
        d.app.playlist.entries[1].selected && d.app.playlist.entries[2].selected,
    );
    d.click_mod(Win::Pl, 60.0, 20.0 + row_h * 0.5, Modifiers::COMMAND);
    d.check(
        "ctrl-click adds row 1",
        d.app.playlist.selected_indices().len() == 3,
    );
    d.time += 1.0; // make sure the previous click can't turn this into a triple-click
    d.double_click(Win::Pl, 60.0, 20.0 + row_h * 2.5);
    println!(
        "     (after double-click: current={:?} sel={:?})",
        d.app.playlist.current,
        d.app.playlist.selected_indices()
    );
    d.check(
        "double-click plays row 3",
        d.app.playlist.current == Some(2),
    );
    d.click(Win::Pl, 60.0, 20.0 + row_h * 0.5);
    let first_title = d.app.playlist.entries[0].title.clone();
    d.drag(
        Win::Pl,
        (60.0, 20.0 + row_h * 0.5),
        (60.0, 20.0 + row_h * 2.9),
    );
    println!(
        "     (order: {:?})",
        d.app
            .playlist
            .entries
            .iter()
            .map(|e| e.title.clone())
            .collect::<Vec<_>>()
    );
    d.check(
        "drag reorders row 1 after row 3",
        d.app.playlist.entries[2].title == first_title,
    );
    d.key(Win::Pl, Key::A, Modifiers::COMMAND);
    d.check(
        "Ctrl+A selects all",
        d.app.playlist.selected_indices().len() == n,
    );
    d.click(Win::Pl, 60.0, 20.0 + row_h * 0.5);
    d.key(Win::Pl, Key::ArrowDown, Modifiers::NONE);
    d.check(
        "Down arrow moves selection",
        d.app.playlist.entries[1].selected && !d.app.playlist.entries[0].selected,
    );
    d.key(Win::Pl, Key::Delete, Modifiers::NONE);
    d.check("Delete removes selected", d.app.playlist.len() == n - 1);
    d.click(Win::Pl, 23.0, 116.0 - 38.0 + 29.0); // ADD button
    d.check("ADD button opens menu", d.app.menu.is_some());
    d.app.menu = None;
    d.drag(Win::Pl, (268.0, 108.0), (268.0 + 60.0, 108.0 + 70.0));
    d.check(
        "resize handle grows playlist",
        d.app.cfg.pl_extra_cols >= 2 && d.app.cfg.pl_extra_rows >= 2,
    );
    let w = d.app.pl_size().x;
    d.click(Win::Pl, w - 25.0 + 9.0, 7.0); // shade button in title bar
    d.check("PL shade button shades", d.app.windows.get(Win::Pl).shaded);
    d.idle(Win::Pl, 2);
    d.click(Win::Pl, w - 50.0 + 34.0, 7.0); // unshade
    d.check(
        "PL unshade button restores",
        !d.app.windows.get(Win::Pl).shaded,
    );

    // Dialog helpers.
    d.check(
        "parse_time 1:30",
        crate::dialogs::parse_time("1:30") == Some(90.0),
    );

    println!();
    d.app.player.stop();
    if d.failures.is_empty() {
        println!("SELFTEST PASSED");
        Ok(())
    } else {
        Err(anyhow!(
            "SELFTEST FAILED: {} failure(s): {:?}",
            d.failures.len(),
            d.failures
        ))
    }
}
