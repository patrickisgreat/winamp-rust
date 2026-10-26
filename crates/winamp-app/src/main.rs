#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod cli;
mod config;
mod dialogs;
mod eq_window;
mod gfx;
mod headless;
mod hotkeys;
mod main_window;
mod menu;
mod milkdrop;
mod pl_window;
mod radio;
mod vis_window;
mod widgets;
mod windows;

use std::process::ExitCode;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use egui::{Vec2, ViewportBuilder, ViewportCommand};

use crate::app::App;
use crate::cli::CliCommand;
use crate::windows::Win;

struct Runner {
    app: App,
}

impl eframe::App for Runner {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let app = &mut self.app;
        app.windows.snap = app.cfg.snap_windows;
        app.windows.lock = app.cfg.lock_windows;
        app.windows.observe(Win::Main, ctx);
        app.update_window_level(ctx);
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
        if app.milk.is_some() {
            let builder = vis_window::builder(app);
            ctx.show_viewport_immediate(vis_window::milk_viewport_id(), builder, |ctx, _class| {
                vis_window::show(app, ctx);
            });
        }
        main_window::show_menu(app, ctx);
        dialogs::show_all(app, ctx);

        ctx.request_repaint_after(Duration::from_millis(if app.milk.is_some() {
            16
        } else {
            33
        }));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.app.save_all();
    }
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let result = match cli::parse_args(std::env::args().skip(1)) {
        CliCommand::ExportDefaultSkin(dir) => winamp_skin::export_builtin(&dir)
            .map(|()| println!("exported built-in skin to {}", dir.display())),
        CliCommand::InstallTheme(name) => install_theme(&name),
        CliCommand::Screenshot { dir, inputs } => headless::run(inputs, &dir),
        CliCommand::SelfTest { inputs } => headless::selftest(inputs),
        CliCommand::Help => {
            println!("{}", cli::USAGE);
            Ok(())
        }
        CliCommand::Run { inputs } => run_player(inputs),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("winamp: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Generate a procedural theme into the skins folder and make it the current skin.
fn install_theme(name: &str) -> Result<()> {
    let theme = winamp_skin::themes::by_name(name).ok_or_else(|| {
        let available: Vec<&str> = winamp_skin::themes::all().iter().map(|t| t.name).collect();
        anyhow!(
            "unknown theme '{name}'; available: {}",
            available.join(", ")
        )
    })?;
    let path =
        winamp_skin::themes::install(theme, &config::skins_dir()).context("installing theme")?;
    let mut cfg = config::Config::load();
    cfg.skin = Some(path.clone());
    cfg.save()?;
    println!(
        "installed theme '{}' to {} and selected it",
        theme.name,
        path.display()
    );
    Ok(())
}

/// Open the main window and run the event loop until the user quits.
fn run_player(inputs: Vec<String>) -> Result<()> {
    let cfg = config::Config::load();
    let scale = if cfg.double_size { 2.0 } else { 1.0 };
    let height = if cfg.main_shaded { 14.0 } else { 116.0 };
    let mut viewport = ViewportBuilder::default()
        .with_title("Winamp")
        .with_inner_size(Vec2::new(275.0 * scale, height * scale))
        .with_decorations(false)
        .with_resizable(false)
        .with_app_id("winamp-rust");
    if let Some(p) = cfg.main_pos {
        viewport = viewport.with_position(egui::Pos2::new(p.x, p.y));
    }
    if cfg.always_on_top {
        viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnTop);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Winamp",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let app = App::new(&cc.egui_ctx, inputs)?;
            cc.egui_ctx
                .send_viewport_cmd(ViewportCommand::Title(app.window_title()));
            Ok(Box::new(Runner { app }))
        }),
    )
    .map_err(|e| anyhow!("{e}"))
}
