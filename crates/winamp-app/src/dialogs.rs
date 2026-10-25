//! Non-skinned dialogs rendered as decorated egui viewports.

use std::path::PathBuf;

use egui::{Color32, ViewportBuilder, ViewportId};
use winamp_audio::{format_duration, Source};

use crate::actions::Cmd;
use crate::app::App;
use crate::config::{AnalyzerStyle, TimeMode, VisMode};

#[derive(Default)]
pub struct JumpDialog {
    pub filter: String,
    pub selected: Option<usize>,
    pub focus_requested: bool,
}

#[derive(Default)]
pub struct UrlDialog {
    pub text: String,
    pub add_only: bool,
}

#[derive(Default)]
pub struct Dialogs {
    pub prefs_open: bool,
    pub prefs_tab: usize,
    pub jump: Option<JumpDialog>,
    pub jump_time: Option<String>,
    pub file_info: Option<usize>,
    pub url: Option<UrlDialog>,
    pub eq_presets_open: bool,
    pub preset_name: String,
    pub preset_selected: Option<String>,
    pub about: bool,
    pub message: Option<String>,
}

impl Dialogs {
    pub fn open_url(&mut self, add_only: bool) {
        self.url = Some(UrlDialog { text: String::new(), add_only });
    }
    pub fn open_jump(&mut self) {
        self.jump = Some(JumpDialog { focus_requested: true, ..Default::default() });
    }
}

fn dialog(ctx: &egui::Context, name: &str, title: &str, size: [f32; 2], f: impl FnOnce(&egui::Context, &mut egui::Ui) -> bool) -> bool {
    let id = ViewportId::from_hash_of(name);
    let builder = ViewportBuilder::default().with_title(title).with_inner_size(size).with_resizable(true);
    let mut keep = true;
    let mut f = Some(f);
    ctx.show_viewport_immediate(id, builder, |ctx, _class| {
        if ctx.input(|i| i.viewport().close_requested()) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            keep = false;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(f) = f.take() {
                if !f(ctx, ui) {
                    keep = false;
                }
            }
        });
    });
    keep
}

pub fn list_skins() -> Vec<PathBuf> {
    let dir = crate::config::skins_dir();
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let ext = p.extension().map(|s| s.to_string_lossy().to_ascii_lowercase());
            if p.is_dir() || matches!(ext.as_deref(), Some("wsz") | Some("zip")) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

pub fn show_all(app: &mut App, ctx: &egui::Context) {
    let mut cmds: Vec<Cmd> = Vec::new();

    // --- Message box -------------------------------------------------------
    if let Some(msg) = app.dialogs.message.clone() {
        let keep = dialog(ctx, "winamp-msg", "Winamp", [360.0, 120.0], |_ctx, ui| {
            ui.add_space(8.0);
            ui.label(&msg);
            ui.add_space(12.0);
            !ui.button("OK").clicked()
        });
        if !keep {
            app.dialogs.message = None;
        }
    }

    // --- Open / Add URL ----------------------------------------------------
    if let Some(d) = &mut app.dialogs.url {
        let mut text = d.text.clone();
        let add_only = d.add_only;
        let mut result: Option<(String, bool)> = None;
        let keep = dialog(ctx, "winamp-url", if add_only { "Add URL" } else { "Open location" }, [420.0, 110.0], |_ctx, ui| {
            ui.label("Enter an Internet location to open (http:// or https://):");
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY).hint_text("http://"));
            if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut keep = true;
            ui.horizontal(|ui| {
                if ui.button(if add_only { "Add" } else { "Open" }).clicked() || enter {
                    result = Some((text.trim().to_string(), add_only));
                    keep = false;
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
            keep
        });
        d.text = text;
        if let Some((url, add_only)) = result {
            if !url.is_empty() {
                let url = if url.contains("://") { url } else { format!("http://{url}") };
                if add_only {
                    app.add_url(&url);
                } else {
                    app.playlist.clear();
                    app.add_url(&url);
                    if !app.playlist.is_empty() {
                        cmds.push(Cmd::PlayIndex(0));
                    }
                }
            }
        }
        if !keep {
            app.dialogs.url = None;
        }
    }

    // --- Jump to file ------------------------------------------------------
    if app.dialogs.jump.is_some() {
        let entries: Vec<(usize, String)> = app.playlist.entries.iter().enumerate().map(|(i, e)| (i, format!("{}. {}", i + 1, e.title))).collect();
        let d = app.dialogs.jump.as_mut().unwrap();
        let mut filter = d.filter.clone();
        let mut selected = d.selected;
        let mut focus = d.focus_requested;
        let mut play: Option<usize> = None;
        let keep = dialog(ctx, "winamp-jump", "Jump to file", [420.0, 380.0], |_ctx, ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut filter).desired_width(f32::INFINITY).hint_text("Type to search"));
            if focus {
                r.request_focus();
                focus = false;
            }
            let words: Vec<String> = filter.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
            let matches: Vec<&(usize, String)> = entries
                .iter()
                .filter(|(_, t)| {
                    let l = t.to_lowercase();
                    words.iter().all(|w| l.contains(w))
                })
                .collect();
            if let Some(s) = selected {
                if !matches.iter().any(|(i, _)| *i == s) {
                    selected = matches.first().map(|(i, _)| *i);
                }
            } else {
                selected = matches.first().map(|(i, _)| *i);
            }
            let (up, down, enter) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter)));
            if up || down {
                if let Some(pos) = matches.iter().position(|(i, _)| Some(*i) == selected) {
                    let np = if up { pos.saturating_sub(1) } else { (pos + 1).min(matches.len().saturating_sub(1)) };
                    selected = matches.get(np).map(|(i, _)| *i);
                }
            }
            if enter {
                play = selected;
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(280.0).auto_shrink([false, false]).show(ui, |ui| {
                for (i, t) in &matches {
                    let r = ui.selectable_label(selected == Some(*i), t);
                    if r.clicked() {
                        selected = Some(*i);
                    }
                    if r.double_clicked() {
                        play = Some(*i);
                    }
                    if selected == Some(*i) && (up || down) {
                        r.scroll_to_me(None);
                    }
                }
            });
            ui.add_space(4.0);
            let mut keep = true;
            ui.horizontal(|ui| {
                if ui.button("Play").clicked() {
                    play = selected;
                }
                if ui.button("Close").clicked() {
                    keep = false;
                }
                ui.label(format!("{} of {} entries", matches.len(), entries.len()));
            });
            keep
        });
        d.filter = filter;
        d.selected = selected;
        d.focus_requested = focus;
        if let Some(i) = play {
            cmds.push(Cmd::PlayIndex(i));
            app.dialogs.jump = None;
        } else if !keep {
            app.dialogs.jump = None;
        }
    }

    // --- Jump to time ------------------------------------------------------
    if let Some(text) = &mut app.dialogs.jump_time {
        let mut t = text.clone();
        let mut go = false;
        let dur = app.player.duration();
        let keep = dialog(ctx, "winamp-jumptime", "Jump to time", [300.0, 110.0], |_ctx, ui| {
            ui.label(format!("Enter time (m:ss){}:", dur.map(|d| format!(", track length {}", format_duration(d))).unwrap_or_default()));
            let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(f32::INFINITY));
            if ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut keep = true;
            ui.horizontal(|ui| {
                if ui.button("Jump").clicked() || enter {
                    go = true;
                    keep = false;
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
            keep
        });
        *text = t.clone();
        if go {
            if let Some(secs) = parse_time(&t) {
                cmds.push(Cmd::SeekTo(secs));
            }
        }
        if !keep {
            app.dialogs.jump_time = None;
        }
    }

    // --- File info ---------------------------------------------------------
    if let Some(idx) = app.dialogs.file_info {
        if idx >= app.playlist.len() {
            app.dialogs.file_info = None;
        } else {
            let e = &app.playlist.entries[idx];
            let title = e.title.clone();
            let path = e.source.as_display();
            let meta = e.meta.clone();
            let info = app.player.info().cloned().filter(|_| app.playlist.current == Some(idx));
            let keep = dialog(ctx, "winamp-fileinfo", "File info", [460.0, 360.0], |_ctx, ui| {
                ui.heading(&title);
                ui.add(egui::TextEdit::singleline(&mut path.clone()).desired_width(f32::INFINITY));
                ui.separator();
                egui::Grid::new("fileinfo").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, k: &str, v: String| {
                        ui.label(k);
                        ui.label(v);
                        ui.end_row();
                    };
                    if let Some(m) = &meta {
                        row(ui, "Title", m.title.clone().unwrap_or_default());
                        row(ui, "Artist", m.artist.clone().unwrap_or_default());
                        row(ui, "Album", m.album.clone().unwrap_or_default());
                        row(ui, "Year", m.year.map(|y| y.to_string()).unwrap_or_default());
                        row(ui, "Track", m.track_number.map(|y| y.to_string()).unwrap_or_default());
                        row(ui, "Genre", m.genre.clone().unwrap_or_default());
                        row(ui, "Comment", m.comment.clone().unwrap_or_default());
                        row(ui, "Length", m.duration.map(format_duration).unwrap_or_default());
                        row(ui, "Bitrate", m.bitrate_kbps.map(|b| format!("{b} kbps")).unwrap_or_default());
                        row(ui, "Sample rate", m.sample_rate.map(|s| format!("{s} Hz")).unwrap_or_default());
                        row(ui, "Channels", m.channels.map(|c| c.to_string()).unwrap_or_default());
                        row(ui, "File size", m.file_size.map(|s| format!("{} KB", s / 1024)).unwrap_or_default());
                    }
                    if let Some(i) = &info {
                        row(ui, "Codec", i.codec.clone());
                        if let Some(s) = &i.stream {
                            row(ui, "Station", s.name.clone().unwrap_or_default());
                            row(ui, "Stream genre", s.genre.clone().unwrap_or_default());
                            row(ui, "Content type", s.content_type.clone().unwrap_or_default());
                        }
                    }
                    if meta.is_none() && info.is_none() {
                        row(ui, "Info", "No metadata available".into());
                    }
                });
                ui.add_space(8.0);
                !ui.button("Close").clicked()
            });
            if !keep {
                app.dialogs.file_info = None;
            }
        }
    }

    // --- EQ presets --------------------------------------------------------
    if app.dialogs.eq_presets_open {
        let user: Vec<String> = app.cfg.presets.iter().map(|p| p.name.clone()).collect();
        let builtin: Vec<String> = winamp_audio::eq::builtin_presets().into_iter().map(|(n, _)| n).collect();
        let mut name = app.dialogs.preset_name.clone();
        let mut selected = app.dialogs.preset_selected.clone();
        let current_file = match app.player.current_source() {
            Some(Source::File(p)) => p.file_name().map(|s| s.to_string_lossy().to_string()),
            _ => None,
        };
        let has_auto = current_file.as_ref().map(|f| app.cfg.auto_presets.iter().any(|p| p.name.eq_ignore_ascii_case(f))).unwrap_or(false);
        let keep = dialog(ctx, "winamp-eqpresets", "Equalizer presets", [420.0, 400.0], |_ctx, ui| {
            ui.horizontal(|ui| {
                ui.label("Preset name:");
                ui.add(egui::TextEdit::singleline(&mut name).desired_width(200.0));
                if ui.button("Save").clicked() && !name.trim().is_empty() {
                    cmds.push(Cmd::SavePreset(name.trim().to_string()));
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(230.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.label(egui::RichText::new("User presets").strong());
                if user.is_empty() {
                    ui.label(egui::RichText::new("(none saved yet)").weak());
                }
                for n in &user {
                    let r = ui.selectable_label(selected.as_deref() == Some(n.as_str()), n);
                    if r.clicked() {
                        selected = Some(n.clone());
                        name = n.clone();
                    }
                    if r.double_clicked() {
                        cmds.push(Cmd::LoadPreset(n.clone()));
                    }
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Built-in presets").strong());
                for (i, n) in builtin.iter().enumerate() {
                    let r = ui.selectable_label(false, n);
                    if r.clicked() || r.double_clicked() {
                        cmds.push(Cmd::LoadBuiltinPreset(i));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(selected.is_some(), egui::Button::new("Load")).clicked() {
                    cmds.push(Cmd::LoadPreset(selected.clone().unwrap()));
                }
                if ui.add_enabled(selected.is_some(), egui::Button::new("Delete")).clicked() {
                    cmds.push(Cmd::DeletePreset(selected.clone().unwrap()));
                    selected = None;
                }
                if ui.button("Flat").clicked() {
                    cmds.push(Cmd::EqFlat);
                }
                if ui.button("Load from file...").clicked() {
                    cmds.push(Cmd::LoadPresetFile);
                }
                if ui.button("Save to file...").clicked() {
                    cmds.push(Cmd::SavePresetFile);
                }
            });
            ui.separator();
            ui.label(egui::RichText::new("Auto-load preset (per file)").strong());
            match &current_file {
                Some(f) => {
                    ui.label(format!("Current file: {f}{}", if has_auto { "  (has auto-preset)" } else { "" }));
                    ui.horizontal(|ui| {
                        if ui.button("Save current EQ for this file").clicked() {
                            cmds.push(Cmd::SaveAutoPreset);
                        }
                        if ui.add_enabled(has_auto, egui::Button::new("Delete auto-preset")).clicked() {
                            cmds.push(Cmd::DeleteAutoPreset);
                        }
                    });
                }
                None => {
                    ui.label(egui::RichText::new("No local file is playing").weak());
                }
            }
            ui.add_space(6.0);
            !ui.button("Close").clicked()
        });
        app.dialogs.preset_name = name;
        app.dialogs.preset_selected = selected;
        if !keep {
            app.dialogs.eq_presets_open = false;
        }
    }

    // --- Preferences -------------------------------------------------------
    if app.dialogs.prefs_open {
        let mut tab = app.dialogs.prefs_tab;
        let mut cfg = app.cfg.clone();
        let mut load_skin: Option<Option<PathBuf>> = None;
        let skins = list_skins();
        let skin_dir = crate::config::skins_dir();
        let out_rate = app.player.output_sample_rate();
        let keep = dialog(ctx, "winamp-prefs", "Winamp Preferences", [560.0, 440.0], |_ctx, ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    for (i, name) in ["General", "Skins", "Display", "Playlist", "Output", "Hotkeys"].iter().enumerate() {
                        if ui.selectable_label(tab == i, *name).clicked() {
                            tab = i;
                        }
                    }
                });
                ui.separator();
                ui.vertical(|ui| match tab {
                    0 => {
                        ui.heading("General");
                        ui.checkbox(&mut cfg.read_tags_on_add, "Read titles/lengths (ID3 tags) when files are added");
                        ui.checkbox(&mut cfg.snap_windows, "Snap windows to each other and to screen edges");
                        ui.checkbox(&mut cfg.always_on_top, "Always on top");
                        ui.checkbox(&mut cfg.double_size, "Double size mode");
                        ui.checkbox(&mut cfg.repeat, "Repeat playlist");
                        ui.checkbox(&mut cfg.shuffle, "Shuffle playlist");
                        ui.add_space(8.0);
                        ui.label(format!("Settings file: {}", crate::config::config_path().display()));
                    }
                    1 => {
                        ui.heading("Skins");
                        ui.label(format!("Skins directory: {}", skin_dir.display()));
                        ui.label("Drop .wsz / .zip classic skins (or unpacked folders) there.");
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().max_height(240.0).auto_shrink([false, false]).show(ui, |ui| {
                            if ui.selectable_label(cfg.skin.is_none(), "<Base Skin>").clicked() {
                                load_skin = Some(None);
                            }
                            for p in &skins {
                                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                if ui.selectable_label(cfg.skin.as_ref() == Some(p), name).clicked() {
                                    load_skin = Some(Some(p.clone()));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Browse for skin file...").clicked() {
                                if let Some(f) = rfd::FileDialog::new().add_filter("Winamp skins", &["wsz", "zip"]).pick_file() {
                                    load_skin = Some(Some(f));
                                }
                            }
                            if ui.button("Open skins folder").clicked() {
                                let _ = std::fs::create_dir_all(&skin_dir);
                                open_folder(&skin_dir);
                            }
                        });
                    }
                    2 => {
                        ui.heading("Display");
                        ui.checkbox(&mut cfg.scroll_title, "Scroll song title in the main window");
                        ui.horizontal(|ui| {
                            ui.label("Time display:");
                            ui.radio_value(&mut cfg.time_mode, TimeMode::Elapsed, "Elapsed");
                            ui.radio_value(&mut cfg.time_mode, TimeMode::Remaining, "Remaining");
                        });
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Visualization").strong());
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut cfg.vis_mode, VisMode::Analyzer, "Spectrum analyzer");
                            ui.radio_value(&mut cfg.vis_mode, VisMode::Scope, "Oscilloscope");
                            ui.radio_value(&mut cfg.vis_mode, VisMode::Off, "Off");
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut cfg.analyzer_style, AnalyzerStyle::Bars, "Bars");
                            ui.radio_value(&mut cfg.analyzer_style, AnalyzerStyle::Lines, "Thin lines");
                        });
                        ui.checkbox(&mut cfg.analyzer_peaks, "Show peaks");
                    }
                    3 => {
                        ui.heading("Playlist");
                        ui.horizontal(|ui| {
                            ui.label("Font size:");
                            ui.add(egui::Slider::new(&mut cfg.playlist_font_size, 7.0..=14.0));
                        });
                        ui.label("Playlist colors and font come from the skin's pledit.txt.");
                    }
                    4 => {
                        ui.heading("Output");
                        ui.label("Output device: system default (via cpal)");
                        ui.label(format!("Output sample rate: {out_rate} Hz"));
                        ui.label("Decoders: symphonia (MP3, AAC/M4A, FLAC, Ogg Vorbis, WAV, AIFF, ALAC, MKV/WebM audio)");
                        ui.label("Streaming: HTTP/HTTPS Shoutcast & Icecast with ICY metadata");
                    }
                    _ => {
                        ui.heading("Keyboard shortcuts");
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            for (k, v) in crate::hotkeys::HELP {
                                ui.horizontal(|ui| {
                                    ui.monospace(format!("{k:<14}"));
                                    ui.label(*v);
                                });
                            }
                        });
                    }
                });
            });
            ui.separator();
            !ui.button("Close").clicked()
        });
        app.dialogs.prefs_tab = tab;
        // Apply changed settings.
        if cfg.always_on_top != app.cfg.always_on_top {
            cmds.push(Cmd::ToggleAlwaysOnTop);
            cfg.always_on_top = app.cfg.always_on_top;
        }
        if cfg.double_size != app.cfg.double_size {
            cmds.push(Cmd::ToggleDoubleSize);
            cfg.double_size = app.cfg.double_size;
        }
        app.cfg = cfg;
        if let Some(s) = load_skin {
            cmds.push(Cmd::LoadSkin(s));
        }
        if !keep {
            app.dialogs.prefs_open = false;
            app.cfg.save();
        }
    }

    // --- About -------------------------------------------------------------
    if app.dialogs.about {
        let keep = dialog(ctx, "winamp-about", "About Winamp", [400.0, 220.0], |_ctx, ui| {
            ui.heading("Winamp 2.95 (Rust rewrite)");
            ui.label("A faithful re-implementation of the classic Winamp 2.x player.");
            ui.label("It really whips the llama's ass.");
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Audio: symphonia + cpal   UI: egui/eframe   Skins: classic .wsz").color(Color32::GRAY));
            ui.add_space(8.0);
            !ui.button("OK").clicked()
        });
        if !keep {
            app.dialogs.about = false;
        }
    }

    for c in cmds {
        app.exec(c, ctx);
    }
}

fn open_folder(p: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
}

/// Parse "m:ss", "h:mm:ss" or plain seconds.
pub fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    let mut total = 0.0;
    for p in parts {
        let v: f64 = p.trim().parse().ok()?;
        total = total * 60.0 + v;
    }
    Some(total)
}

