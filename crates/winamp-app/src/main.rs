#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod config;
mod dialogs;
mod eq_window;
mod gfx;
mod headless;
mod hotkeys;
mod main_window;
mod menu;
mod pl_window;
mod widgets;
mod windows;

use std::time::Duration;

use egui::{Vec2, ViewportBuilder, ViewportCommand};

use crate::app::App;
use crate::windows::Win;

struct Runner {
    app: App,
}

impl eframe::App for Runner {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let app = &mut self.app;
        app.windows.snap = app.cfg.snap_windows;
        app.windows.observe(Win::Main, ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !app.pending_exit {
            app.save_all();
            app.pending_exit = true;
        }
        app.poll(ctx);
        app.update_vis();
        app.tick_marquee();

        main_window::show(app, ctx);
        hotkeys::handle(app, ctx, Win::Main);

        let main_known = app.windows.get(Win::Main).rect.is_some();
        for w in [Win::Eq, Win::Pl] {
            if !app.windows.get(w).visible || !main_known {
                continue;
            }
            let builder = app.child_builder(w);
            ctx.show_viewport_immediate(w.viewport_id(), builder, |ctx, _class| {
                match w {
                    Win::Eq => eq_window::show(app, ctx),
                    Win::Pl => pl_window::show(app, ctx),
                    Win::Main => {}
                }
                hotkeys::handle(app, ctx, w);
            });
        }
        main_window::show_menu(app, ctx);
        dialogs::show_all(app, ctx);

        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.app.save_all();
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--export-default-skin") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "base-skin".to_string());
        match winamp_skin::export_builtin(std::path::Path::new(&dir)) {
            Ok(()) => println!("exported built-in skin to {dir}"),
            Err(e) => eprintln!("export failed: {e}"),
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--install-theme") {
        let name = args.get(i + 1).cloned().unwrap_or_else(|| "DMT".to_string());
        match winamp_skin::themes::by_name(&name) {
            Some(theme) => match winamp_skin::themes::install(theme, &config::skins_dir()) {
                Ok(path) => {
                    let mut cfg = config::Config::load();
                    cfg.skin = Some(path.clone());
                    cfg.save();
                    println!("installed theme '{}' to {} and selected it", theme.name, path.display());
                }
                Err(e) => eprintln!("install failed: {e}"),
            },
            None => eprintln!("unknown theme '{name}'; available: {}", winamp_skin::themes::all().iter().map(|t| t.name).collect::<Vec<_>>().join(", ")),
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--screenshot") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "screenshots".to_string());
        let rest: Vec<String> = args.iter().enumerate().filter(|(k, a)| *k != i && *k != i + 1 && !a.starts_with("--")).map(|(_, a)| a.clone()).collect();
        headless::run(rest, std::path::Path::new(&dir));
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest") {
        let rest: Vec<String> = args.iter().enumerate().filter(|(k, a)| *k != i && !a.starts_with("--")).map(|(_, a)| a.clone()).collect();
        headless::selftest(rest);
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("winamp [--install-theme NAME] [--export-default-skin DIR] [--screenshot DIR] [--selftest] [FILE|DIR|PLAYLIST|URL ...]");
        return;
    }
    args.retain(|a| !a.starts_with("--"));

    let cfg = config::Config::load();
    let scale = if cfg.double_size { 2.0 } else { 1.0 };
    let h = if cfg.main_shaded { 14.0 } else { 116.0 };
    let mut viewport = ViewportBuilder::default()
        .with_title("Winamp")
        .with_inner_size(Vec2::new(275.0 * scale, h * scale))
        .with_decorations(false)
        .with_resizable(false)
        .with_app_id("winamp-rust");
    if let Some(p) = cfg.main_pos {
        viewport = viewport.with_position(egui::Pos2::new(p.x, p.y));
    }
    if cfg.always_on_top {
        viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnTop);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let result = eframe::run_native(
        "Winamp",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let app = App::new(&cc.egui_ctx, args);
            cc.egui_ctx.send_viewport_cmd(ViewportCommand::Title(app.window_title()));
            Ok(Box::new(Runner { app }))
        }),
    );
    if let Err(e) = result {
        eprintln!("winamp: {e}");
    }
}
