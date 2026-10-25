//! Application state and command dispatch.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use egui::{Pos2, Vec2, ViewportBuilder, ViewportCommand, WindowLevel};
use winamp_audio::{format_duration, EqSettings, Event, PlayState, Player, Source, TrackMeta};
use winamp_playlist::{Playlist, SortKey};
use winamp_skin::sprites;
use winamp_skin::Skin;

use crate::actions::Cmd;
use crate::config::{AnalyzerStyle, Config, Preset, TimeMode, VisMode, WindowPos};
use crate::dialogs::Dialogs;
use crate::gfx::{PixelSurface, Textures};
use crate::menu::Menu;
use crate::windows::{Win, Windows};

/// What the mouse is currently pressing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hot {
    Button(Win, u16),
    Slider(Win, u16),
    Marquee(f32),
    PlScroll(f32),
    PlResize(Vec2),
    PlDragRows(usize),
    WindowDrag(Win),
}

impl Hot {
    /// The window that owns this interaction.
    pub fn owner(&self) -> Win {
        match self {
            Hot::Button(w, _) | Hot::Slider(w, _) | Hot::WindowDrag(w) => *w,
            Hot::Marquee(_) => Win::Main,
            Hot::PlScroll(_) | Hot::PlResize(_) | Hot::PlDragRows(_) => Win::Pl,
        }
    }
}

pub struct VisState {
    pub bars: Vec<f32>,
    pub peaks: Vec<f32>,
    pub peak_hold: Vec<u8>,
    pub surface: PixelSurface,
    pub eq_graph: PixelSurface,
}

pub struct App {
    pub player: Player,
    pub playlist: Playlist,
    pub skin: Rc<Skin>,
    pub textures: Rc<Textures>,
    pub vis_samples_cache: Vec<f32>,
    pub cfg: Config,
    pub windows: Windows,
    pub hot: Option<Hot>,
    pub menu: Option<(Win, Menu)>,
    pub dialogs: Dialogs,
    pub vis: VisState,
    pub marquee_offset: i32,
    pub marquee_last: Instant,
    pub status: Option<(String, Instant)>,
    pub eq_auto: bool,
    pub main_hidden: bool,
    pub pl_scroll: usize,
    pub pl_anchor: Option<usize>,
    tags_tx: Sender<Source>,
    tags_rx: Receiver<(Source, TrackMeta)>,
    consecutive_errors: usize,
    last_save: Instant,
    pub started: Instant,
    pub pending_exit: bool,
    initial_pos: [Option<Pos2>; 3],
}

pub const MARQUEE_STEP_MS: u64 = 220;

impl App {
    pub fn new(egui_ctx: &egui::Context, args: Vec<String>) -> Self {
        let cfg = Config::load();
        let player = Player::new().expect("audio output");
        player.set_volume(cfg.volume);
        player.set_balance(cfg.balance);
        player.set_eq(cfg.eq.to_settings());

        let skin = cfg
            .skin
            .as_ref()
            .and_then(|p| match Skin::load(p) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::warn!("skin {}: {e}", p.display());
                    None
                }
            })
            .unwrap_or_else(Skin::builtin);
        let textures = Rc::new(Textures::from_skin(egui_ctx, &skin));
        let skin = Rc::new(skin);

        let (tags_tx, job_rx) = mpsc::channel::<Source>();
        let (res_tx, tags_rx) = mpsc::channel::<(Source, TrackMeta)>();
        std::thread::Builder::new()
            .name("winamp-tags".into())
            .spawn(move || {
                while let Ok(src) = job_rx.recv() {
                    if let Source::File(p) = &src {
                        let meta = winamp_audio::read_meta(p);
                        if res_tx.send((src, meta)).is_err() {
                            break;
                        }
                    }
                }
            })
            .expect("spawn tag thread");

        let mut windows = Windows::new();
        windows.get_mut(Win::Main).visible = true;
        windows.get_mut(Win::Main).shaded = cfg.main_shaded;
        windows.get_mut(Win::Eq).visible = cfg.eq_visible;
        windows.get_mut(Win::Eq).shaded = cfg.eq_shaded;
        windows.get_mut(Win::Pl).visible = cfg.pl_visible;
        windows.get_mut(Win::Pl).shaded = cfg.pl_shaded;

        let vis = VisState {
            bars: vec![0.0; 75],
            peaks: vec![0.0; 75],
            peak_hold: vec![0; 75],
            surface: PixelSurface::new("vis", 76, 16),
            eq_graph: PixelSurface::new("eqgraph", 113, 19),
        };

        let mut app = Self {
            player,
            playlist: Playlist::new(),
            skin,
            textures,
            vis_samples_cache: vec![0.0; 576],
            windows,
            hot: None,
            menu: None,
            dialogs: Dialogs::default(),
            vis,
            marquee_offset: 0,
            marquee_last: Instant::now(),
            status: None,
            eq_auto: cfg.eq.auto,
            main_hidden: false,
            pl_scroll: 0,
            pl_anchor: None,
            tags_tx,
            tags_rx,
            consecutive_errors: 0,
            last_save: Instant::now(),
            started: Instant::now(),
            pending_exit: false,
            initial_pos: [None; 3],
            cfg,
        };
        app.initial_pos = [
            app.cfg.main_pos.map(|p| Pos2::new(p.x, p.y)),
            app.cfg.eq_pos.map(|p| Pos2::new(p.x, p.y)),
            app.cfg.pl_pos.map(|p| Pos2::new(p.x, p.y)),
        ];
        app.load_saved_playlist();
        let mut play_first_new = false;
        let before = app.playlist.len();
        for a in &args {
            let src = Source::from_str(a);
            match src {
                Source::Url(u) => app.add_url(&u),
                Source::File(p) => {
                    app.add_path(&p);
                }
            }
            play_first_new = true;
        }
        if play_first_new && app.playlist.len() > before {
            app.play_index(before);
        }
        app
    }

    // --- persistence --------------------------------------------------------

    fn load_saved_playlist(&mut self) {
        let p = crate::config::playlist_path();
        if p.exists() {
            if let Err(e) = self.playlist.load_file(&p) {
                log::warn!("loading saved playlist: {e}");
            }
            self.playlist.path = None;
            self.queue_tags_for_all();
        }
    }

    pub fn save_all(&mut self) {
        let p = crate::config::playlist_path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let keep = self.playlist.path.take();
        let _ = self.playlist.save_file(&p);
        self.playlist.path = keep;
        self.cfg.volume = self.player.volume();
        self.cfg.balance = self.player.balance();
        self.cfg.eq.from_settings(&self.player.eq());
        self.cfg.eq.auto = self.eq_auto;
        for w in Win::ALL {
            let info = self.windows.get(w);
            let pos = info.rect.map(|r| WindowPos { x: r.min.x, y: r.min.y });
            match w {
                Win::Main => {
                    self.cfg.main_pos = pos.or(self.cfg.main_pos);
                    self.cfg.main_shaded = info.shaded;
                }
                Win::Eq => {
                    self.cfg.eq_pos = pos.or(self.cfg.eq_pos);
                    self.cfg.eq_shaded = info.shaded;
                    self.cfg.eq_visible = info.visible;
                }
                Win::Pl => {
                    self.cfg.pl_pos = pos.or(self.cfg.pl_pos);
                    self.cfg.pl_shaded = info.shaded;
                    self.cfg.pl_visible = info.visible;
                }
            }
        }
        self.cfg.save();
        self.last_save = Instant::now();
    }

    // --- helpers ------------------------------------------------------------

    pub fn scale(&self) -> f32 {
        if self.cfg.double_size {
            2.0
        } else {
            1.0
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now()));
    }

    pub fn queue_tags(&self, src: &Source) {
        if self.cfg.read_tags_on_add {
            let _ = self.tags_tx.send(src.clone());
        }
    }

    pub fn queue_tags_for_all(&self) {
        for e in &self.playlist.entries {
            if e.meta.is_none() {
                self.queue_tags(&e.source);
            }
        }
    }

    pub fn add_path(&mut self, path: &Path) -> usize {
        let before = self.playlist.len();
        let n = self.playlist.add_path(path);
        for e in &self.playlist.entries[before..] {
            self.queue_tags(&e.source);
        }
        n
    }

    pub fn add_url(&mut self, url: &str) {
        let lower = url.to_ascii_lowercase();
        if lower.ends_with(".pls") || lower.ends_with(".m3u") || lower.ends_with(".m3u8") {
            match winamp_audio::stream::fetch_text(url, 256 * 1024) {
                Ok(text) => {
                    let entries = winamp_playlist::parse_remote_playlist(&text, url);
                    if entries.is_empty() {
                        self.playlist.add_url(url);
                    } else {
                        for e in entries {
                            self.playlist.push(e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("remote playlist: {e}");
                    self.playlist.add_url(url);
                }
            }
        } else {
            self.playlist.add_url(url);
        }
    }

    pub fn play_index(&mut self, i: usize) {
        if i >= self.playlist.len() {
            return;
        }
        self.playlist.set_current(i);
        let src = self.playlist.entries[i].source.clone();
        if src.is_url() {
            self.set_status("Connecting...");
        }
        self.player.load(src.clone(), true);
        self.marquee_offset = 0;
        self.apply_auto_eq(&src);
        self.ensure_current_visible();
    }

    fn apply_auto_eq(&mut self, src: &Source) {
        if !self.eq_auto {
            return;
        }
        if let Source::File(p) = src {
            if let Some(preset) = self.cfg.auto_preset_for(p) {
                let mut s = preset.to_settings();
                s.enabled = self.player.eq().enabled;
                self.player.set_eq(s);
            }
        }
    }

    pub fn next_track(&mut self, auto: bool) {
        let (sh, rp) = (self.cfg.shuffle, self.cfg.repeat);
        match self.playlist.next_index(sh, rp) {
            Some(i) => self.play_index(i),
            None => {
                if auto {
                    self.player.stop();
                }
            }
        }
    }

    pub fn prev_track(&mut self) {
        let (sh, rp) = (self.cfg.shuffle, self.cfg.repeat);
        if let Some(i) = self.playlist.prev_index(sh, rp) {
            self.play_index(i);
        }
    }

    pub fn ensure_current_visible(&mut self) {
        if let Some(c) = self.playlist.current {
            let rows = self.pl_visible_rows();
            if c < self.pl_scroll || c >= self.pl_scroll + rows {
                self.pl_scroll = c.saturating_sub(rows / 2);
            }
        }
    }

    pub fn pl_size(&self) -> Vec2 {
        Vec2::new(275.0 + 25.0 * self.cfg.pl_extra_cols as f32, 116.0 + 29.0 * self.cfg.pl_extra_rows as f32)
    }

    pub fn pl_row_h(&self) -> f32 {
        (self.cfg.playlist_font_size + 4.0).round()
    }

    pub fn pl_visible_rows(&self) -> usize {
        let h = self.pl_size().y - 20.0 - 38.0;
        (h / self.pl_row_h()).floor().max(1.0) as usize
    }

    /// Text shown in the main window marquee.
    pub fn marquee_text(&self) -> String {
        if let Some((msg, at)) = &self.status {
            if at.elapsed() < Duration::from_secs(3) {
                return msg.clone();
            }
        }
        match self.playlist.current {
            Some(i) if i < self.playlist.len() => {
                let e = &self.playlist.entries[i];
                let title = self
                    .player
                    .info()
                    .and_then(|inf| if e.source.is_url() { inf.title.clone() } else { None })
                    .unwrap_or_else(|| e.title.clone());
                let dur = self.player.duration().or(e.duration);
                match dur {
                    Some(d) => format!("{}. {} ({})", i + 1, title, format_duration(d)),
                    None => format!("{}. {}", i + 1, title),
                }
            }
            _ => "Winamp 2.95 (Rust)".to_string(),
        }
    }

    pub fn window_title(&self) -> String {
        match self.playlist.current {
            Some(i) if i < self.playlist.len() => format!("{} - Winamp", self.playlist.entries[i].title),
            _ => "Winamp".to_string(),
        }
    }

    // --- per-frame ----------------------------------------------------------

    pub fn poll(&mut self, ctx: &egui::Context) {
        for ev in self.player.poll_events() {
            if std::env::var_os("WINAMP_DEBUG").is_some() {
                eprintln!("event: {ev:?}");
            }
            match ev {
                Event::Opened(info) => {
                    self.consecutive_errors = 0;
                    self.status = None;
                    if let Some(c) = self.playlist.current {
                        if let Some(e) = self.playlist.entries.get_mut(c) {
                            if e.duration.is_none() {
                                e.duration = info.duration;
                            }
                            if e.source.is_url() {
                                if let Some(t) = &info.title {
                                    e.title = t.clone();
                                }
                            }
                        }
                    }
                    ctx.send_viewport_cmd(ViewportCommand::Title(self.window_title()));
                }
                Event::Finished => self.next_track(true),
                Event::Error(e) => {
                    log::warn!("playback error: {e}");
                    self.set_status(format!("Error: {e}"));
                    self.consecutive_errors += 1;
                    if self.consecutive_errors < self.playlist.len().max(1) {
                        self.next_track(true);
                    } else {
                        self.consecutive_errors = 0;
                        self.player.stop();
                    }
                }
                Event::StreamTitle(t) => {
                    if let Some(c) = self.playlist.current {
                        if let Some(e) = self.playlist.entries.get_mut(c) {
                            if e.source.is_url() {
                                e.title = t;
                            }
                        }
                    }
                    self.marquee_offset = 0;
                    ctx.send_viewport_cmd(ViewportCommand::Title(self.window_title()));
                }
            }
        }
        while let Ok((src, meta)) = self.tags_rx.try_recv() {
            for e in self.playlist.entries.iter_mut().filter(|e| e.source == src) {
                if let Source::File(p) = &e.source {
                    e.title = meta.display_title(p);
                }
                if e.duration.is_none() {
                    e.duration = meta.duration;
                }
                e.meta = Some(meta.clone());
            }
        }
        // Dropped files on any window.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            let before = self.playlist.len();
            for p in &dropped {
                self.add_path(p);
            }
            if self.player.state() != PlayState::Playing && self.playlist.len() > before {
                self.play_index(before);
            }
        }
        if self.last_save.elapsed() > Duration::from_secs(60) {
            self.save_all();
        }
    }

    pub fn update_vis(&mut self) {
        let playing = self.player.state() == PlayState::Playing;
        let n = match self.cfg.analyzer_style {
            AnalyzerStyle::Bars => 19,
            AnalyzerStyle::Lines => 75,
        };
        let samples = if playing { self.player.vis_samples(winamp_audio::vis::VIS_SAMPLES) } else { vec![0.0; 576] };
        let spec = winamp_audio::vis::spectrum(&samples, self.player.output_sample_rate(), n);
        for i in 0..n {
            let target = spec[i] * 16.0;
            let cur = self.vis.bars[i];
            self.vis.bars[i] = if target > cur { target } else { (cur - 0.9).max(0.0) };
            if self.vis.bars[i] >= self.vis.peaks[i] {
                self.vis.peaks[i] = self.vis.bars[i];
                self.vis.peak_hold[i] = 20;
            } else if self.vis.peak_hold[i] > 0 {
                self.vis.peak_hold[i] -= 1;
            } else {
                self.vis.peaks[i] = (self.vis.peaks[i] - 0.5).max(0.0);
            }
        }
        if !playing {
            for i in 0..75 {
                self.vis.bars[i] = (self.vis.bars[i] - 1.0).max(0.0);
                self.vis.peaks[i] = (self.vis.peaks[i] - 1.0).max(0.0);
            }
        }
        self.vis_samples_cache = samples;
    }

    pub fn tick_marquee(&mut self) {
        if !self.cfg.scroll_title {
            self.marquee_offset = 0;
            return;
        }
        if self.marquee_last.elapsed() >= Duration::from_millis(MARQUEE_STEP_MS) {
            self.marquee_last = Instant::now();
            if !matches!(self.hot, Some(Hot::Marquee(_))) {
                self.marquee_offset += 1;
            }
        }
    }

    // --- command dispatch ---------------------------------------------------

    pub fn exec(&mut self, cmd: Cmd, ctx: &egui::Context) {
        if std::env::var_os("WINAMP_DEBUG").is_some() {
            eprintln!("exec: {cmd:?}");
        }
        match cmd {
            Cmd::Nop => {}
            Cmd::Prev => self.prev_track(),
            Cmd::Play => {
                if self.playlist.current.is_none() || self.player.current_source().is_none() {
                    if !self.playlist.is_empty() {
                        let i = if self.cfg.shuffle { self.playlist.next_index(true, true).unwrap_or(0) } else { 0 };
                        self.play_index(i);
                    } else {
                        self.exec(Cmd::OpenFiles, ctx);
                    }
                } else {
                    self.player.play();
                }
            }
            Cmd::Pause => self.player.pause(),
            Cmd::Stop => self.player.stop(),
            Cmd::Next => self.next_track(false),
            Cmd::SeekRelative(d) => {
                let pos = self.player.position().as_secs_f64() + d;
                self.player.seek(pos.max(0.0));
            }
            Cmd::SeekTo(t) => self.player.seek(t),
            Cmd::VolumeDelta(d) => self.player.set_volume(self.player.volume() + d),
            Cmd::SetVolume(v) => self.player.set_volume(v),
            Cmd::SetBalance(b) => self.player.set_balance(b),
            Cmd::ToggleShuffle => self.cfg.shuffle = !self.cfg.shuffle,
            Cmd::ToggleRepeat => self.cfg.repeat = !self.cfg.repeat,
            Cmd::PlayIndex(i) => self.play_index(i),

            Cmd::OpenFiles | Cmd::AddFiles => {
                let add_only = cmd == Cmd::AddFiles;
                if let Some(files) = file_dialog().pick_files() {
                    self.load_paths(files, add_only);
                }
            }
            Cmd::OpenDirectory | Cmd::AddDirectory => {
                let add_only = cmd == Cmd::AddDirectory;
                if let Some(dir) = rfd::FileDialog::new().set_title("Open directory").pick_folder() {
                    self.load_paths(vec![dir], add_only);
                }
            }
            Cmd::OpenUrl => self.dialogs.open_url(false),
            Cmd::AddUrl => self.dialogs.open_url(true),

            Cmd::ToggleMain => {
                self.main_hidden = !self.main_hidden;
                ctx.send_viewport_cmd(ViewportCommand::Visible(!self.main_hidden));
            }
            Cmd::ToggleEq => {
                let w = self.windows.get_mut(Win::Eq);
                w.visible = !w.visible;
            }
            Cmd::TogglePlaylist => {
                let w = self.windows.get_mut(Win::Pl);
                w.visible = !w.visible;
            }
            Cmd::ToggleMainShade => {
                let w = self.windows.get_mut(Win::Main);
                w.shaded = !w.shaded;
                let h = if w.shaded { sprites::SHADE_H } else { sprites::MAIN_H } as f32;
                let s = self.scale();
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(275.0 * s, h * s)));
            }
            Cmd::ToggleEqShade => {
                let w = self.windows.get_mut(Win::Eq);
                w.shaded = !w.shaded;
            }
            Cmd::TogglePlShade => {
                let w = self.windows.get_mut(Win::Pl);
                w.shaded = !w.shaded;
            }
            Cmd::ToggleDoubleSize => {
                self.cfg.double_size = !self.cfg.double_size;
                let s = self.scale();
                let h = if self.windows.get(Win::Main).shaded { sprites::SHADE_H } else { sprites::MAIN_H } as f32;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(275.0 * s, h * s)));
            }
            Cmd::ToggleAlwaysOnTop => {
                self.cfg.always_on_top = !self.cfg.always_on_top;
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(self.window_level()));
            }
            Cmd::MinimizeAll => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            Cmd::Exit => {
                self.save_all();
                self.pending_exit = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }

            Cmd::ToggleTimeMode => {
                self.cfg.time_mode = match self.cfg.time_mode {
                    TimeMode::Elapsed => TimeMode::Remaining,
                    TimeMode::Remaining => TimeMode::Elapsed,
                }
            }
            Cmd::CycleVisMode => {
                self.cfg.vis_mode = match self.cfg.vis_mode {
                    VisMode::Analyzer => VisMode::Scope,
                    VisMode::Scope => VisMode::Off,
                    VisMode::Off => VisMode::Analyzer,
                }
            }
            Cmd::SetVisMode(m) => self.cfg.vis_mode = m,
            Cmd::ToggleAnalyzerStyle => {
                self.cfg.analyzer_style = match self.cfg.analyzer_style {
                    AnalyzerStyle::Bars => AnalyzerStyle::Lines,
                    AnalyzerStyle::Lines => AnalyzerStyle::Bars,
                };
                self.vis.bars.iter_mut().for_each(|b| *b = 0.0);
                self.vis.peaks.iter_mut().for_each(|b| *b = 0.0);
            }
            Cmd::ToggleAnalyzerPeaks => self.cfg.analyzer_peaks = !self.cfg.analyzer_peaks,
            Cmd::ToggleScrollTitle => self.cfg.scroll_title = !self.cfg.scroll_title,

            Cmd::ToggleEqEnabled => {
                let mut s = self.player.eq();
                s.enabled = !s.enabled;
                self.player.set_eq(s);
            }
            Cmd::ToggleEqAuto => {
                self.eq_auto = !self.eq_auto;
                if self.eq_auto {
                    if let Some(src) = self.player.current_source().cloned() {
                        self.apply_auto_eq(&src);
                    }
                }
            }
            Cmd::EqPresetsDialog => self.dialogs.eq_presets_open = true,
            Cmd::LoadPreset(name) => {
                if let Some(p) = self.cfg.find_preset(&name).cloned() {
                    let mut s = p.to_settings();
                    s.enabled = self.player.eq().enabled;
                    self.player.set_eq(s);
                }
            }
            Cmd::LoadBuiltinPreset(i) => {
                let presets = winamp_audio::eq::builtin_presets();
                if let Some((_, s)) = presets.get(i) {
                    let mut s = *s;
                    s.enabled = self.player.eq().enabled;
                    self.player.set_eq(s);
                }
            }
            Cmd::SavePreset(name) => {
                let s = self.player.eq();
                let p = Preset { name: name.clone(), preamp: s.preamp_db, bands: s.bands_db };
                if let Some(existing) = self.cfg.presets.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&name)) {
                    *existing = p;
                } else {
                    self.cfg.presets.push(p);
                }
                self.cfg.presets.sort_by_key(|p| p.name.to_lowercase());
                self.cfg.save();
            }
            Cmd::DeletePreset(name) => {
                self.cfg.presets.retain(|p| !p.name.eq_ignore_ascii_case(&name));
                self.cfg.save();
            }
            Cmd::SaveAutoPreset => {
                if let Some(Source::File(p)) = self.player.current_source().cloned() {
                    let key = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let s = self.player.eq();
                    let preset = Preset { name: key.clone(), preamp: s.preamp_db, bands: s.bands_db };
                    self.cfg.auto_presets.retain(|p| !p.name.eq_ignore_ascii_case(&key));
                    self.cfg.auto_presets.push(preset);
                    self.cfg.save();
                    self.set_status(format!("Saved auto-preset for {key}"));
                }
            }
            Cmd::DeleteAutoPreset => {
                if let Some(Source::File(p)) = self.player.current_source().cloned() {
                    let key = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    self.cfg.auto_presets.retain(|p| !p.name.eq_ignore_ascii_case(&key));
                    self.cfg.save();
                }
            }
            Cmd::LoadPresetFile => {
                if let Some(f) = rfd::FileDialog::new().add_filter("EQ presets", &["eqf", "q1"]).pick_file() {
                    match std::fs::read(&f) {
                        Ok(bytes) => {
                            let presets = winamp_audio::eq::parse_eqf(&bytes);
                            if presets.is_empty() {
                                self.dialogs.message = Some("Not a valid EQF file".into());
                            } else if presets.len() == 1 {
                                let mut s = presets[0].1;
                                s.enabled = self.player.eq().enabled;
                                self.player.set_eq(s);
                            } else {
                                for (name, s) in presets {
                                    self.cfg.presets.retain(|p| !p.name.eq_ignore_ascii_case(&name));
                                    self.cfg.presets.push(Preset { name, preamp: s.preamp_db, bands: s.bands_db });
                                }
                                self.cfg.presets.sort_by_key(|p| p.name.to_lowercase());
                                self.cfg.save();
                            }
                        }
                        Err(e) => self.dialogs.message = Some(format!("Could not read file: {e}")),
                    }
                }
            }
            Cmd::SavePresetFile => {
                if let Some(f) = rfd::FileDialog::new().add_filter("EQ preset", &["eqf"]).set_file_name("preset.eqf").save_file() {
                    let s = self.player.eq();
                    let bytes = winamp_audio::eq::write_eqf(&[("Entry1".to_string(), s)]);
                    if let Err(e) = std::fs::write(&f, bytes) {
                        self.dialogs.message = Some(format!("Could not save: {e}"));
                    }
                }
            }
            Cmd::EqFlat => {
                let enabled = self.player.eq().enabled;
                self.player.set_eq(EqSettings { enabled, ..EqSettings::flat() });
            }
            Cmd::SetPreamp(v) => {
                let mut s = self.player.eq();
                s.preamp_db = v;
                self.player.set_eq(s);
            }
            Cmd::SetBand(i, v) => {
                let mut s = self.player.eq();
                if i < 10 {
                    s.bands_db[i] = v;
                }
                self.player.set_eq(s);
            }

            Cmd::PlRemoveSelected => self.playlist.remove_selected(),
            Cmd::PlRemoveAll => {
                self.playlist.clear();
                self.pl_scroll = 0;
            }
            Cmd::PlCrop => self.playlist.crop(),
            Cmd::PlRemoveMissing => self.playlist.remove_missing_files(),
            Cmd::PlRemoveDuplicates => self.playlist.remove_duplicates(),
            Cmd::PlSelectAll => self.playlist.select_all(),
            Cmd::PlSelectNone => self.playlist.select_none(),
            Cmd::PlInvertSelection => self.playlist.invert_selection(),
            Cmd::PlSort(k) => self.playlist.sort(k, false),
            Cmd::PlReverse => self.playlist.reverse(),
            Cmd::PlRandomize => self.playlist.randomize(),
            Cmd::PlMoveSelected(d) => self.playlist.move_selected(d),
            Cmd::PlNew => {
                self.playlist.clear();
                self.playlist.path = None;
                self.pl_scroll = 0;
            }
            Cmd::PlSave => {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter("Playlists", &["m3u8", "m3u", "pls"])
                    .set_file_name("playlist.m3u8");
                if let Some(p) = &self.playlist.path {
                    if let Some(dir) = p.parent() {
                        dlg = dlg.set_directory(dir);
                    }
                }
                if let Some(f) = dlg.save_file() {
                    if let Err(e) = self.playlist.save_file(&f) {
                        self.dialogs.message = Some(format!("Could not save playlist: {e}"));
                    }
                }
            }
            Cmd::PlLoad => {
                if let Some(f) = rfd::FileDialog::new().add_filter("Playlists", &["m3u8", "m3u", "pls"]).pick_file() {
                    match self.playlist.load_file(&f) {
                        Ok(()) => {
                            self.pl_scroll = 0;
                            self.queue_tags_for_all();
                        }
                        Err(e) => self.dialogs.message = Some(format!("Could not load playlist: {e}")),
                    }
                }
            }
            Cmd::PlPlaySelected => {
                if let Some(i) = self.playlist.selected_indices().first().copied() {
                    self.play_index(i);
                }
            }
            Cmd::PlFileInfo => {
                let idx = self.playlist.selected_indices().first().copied().or(self.playlist.current);
                if let Some(i) = idx {
                    self.dialogs.file_info = Some(i);
                }
            }

            Cmd::JumpToFile => self.dialogs.open_jump(),
            Cmd::JumpToTime => self.dialogs.jump_time = Some(String::new()),
            Cmd::FileInfo => {
                if let Some(c) = self.playlist.current {
                    self.dialogs.file_info = Some(c);
                }
            }
            Cmd::Preferences => self.dialogs.prefs_open = true,
            Cmd::SkinBrowser => {
                self.dialogs.prefs_open = true;
                self.dialogs.prefs_tab = 1;
            }
            Cmd::About => self.dialogs.about = true,
            Cmd::LoadSkin(path) => self.load_skin(path, ctx),
        }
    }

    fn load_paths(&mut self, paths: Vec<PathBuf>, add_only: bool) {
        if paths.is_empty() {
            return;
        }
        if !add_only {
            self.playlist.clear();
            self.pl_scroll = 0;
        }
        let before = self.playlist.len();
        for p in &paths {
            self.add_path(p);
        }
        if !add_only && self.playlist.len() > before {
            self.play_index(before);
        }
    }

    pub fn load_skin(&mut self, path: Option<PathBuf>, ctx: &egui::Context) {
        let skin = match &path {
            None => Skin::builtin(),
            Some(p) => match Skin::load(p) {
                Ok(s) => s,
                Err(e) => {
                    self.dialogs.message = Some(format!("Could not load skin: {e}"));
                    return;
                }
            },
        };
        self.textures = Rc::new(Textures::from_skin(ctx, &skin));
        self.skin = Rc::new(skin);
        self.cfg.skin = path;
        self.cfg.save();
    }

    pub fn window_level(&self) -> WindowLevel {
        if self.cfg.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        }
    }

    /// Builder for the EQ / playlist viewports (re-evaluated every frame; egui
    /// only sends commands for fields that changed).
    pub fn child_builder(&mut self, w: Win) -> ViewportBuilder {
        let s = self.scale();
        let info = *self.windows.get(w);
        let size = match (w, info.shaded) {
            (Win::Eq, false) => Vec2::new(275.0, 116.0),
            (Win::Eq, true) => Vec2::new(275.0, 14.0),
            (Win::Pl, false) => self.pl_size(),
            (Win::Pl, true) => Vec2::new(self.pl_size().x, 14.0),
            (Win::Main, _) => Vec2::new(275.0, 116.0),
        };
        let idx = match w {
            Win::Main => 0,
            Win::Eq => 1,
            Win::Pl => 2,
        };
        if self.initial_pos[idx].is_none() {
            self.initial_pos[idx] = Some(self.windows.default_position(w));
        }
        let title = match w {
            Win::Eq => "Winamp Equalizer",
            Win::Pl => "Winamp Playlist Editor",
            Win::Main => "Winamp",
        };
        ViewportBuilder::default()
            .with_title(title)
            .with_decorations(false)
            .with_resizable(false)
            .with_inner_size(size * s)
            .with_position(self.initial_pos[idx].unwrap())
            .with_window_level(self.window_level())
            .with_taskbar(false)
    }
}

pub fn file_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_title("Open file(s)")
        .add_filter("Audio files", winamp_audio::SUPPORTED_EXTENSIONS)
        .add_filter("Playlists", &["m3u", "m3u8", "pls"])
        .add_filter("All files", &["*"])
}

pub fn sort_key_label(k: SortKey) -> &'static str {
    match k {
        SortKey::Title => "Sort list by title",
        SortKey::Filename => "Sort list by filename",
        SortKey::Path => "Sort list by path and filename",
    }
}
