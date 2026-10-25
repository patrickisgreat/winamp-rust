//! Persistent settings (the equivalent of winamp.ini) stored as TOML.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use winamp_audio::EqSettings;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum VisMode {
    #[default]
    Analyzer,
    Scope,
    Off,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AnalyzerStyle {
    #[default]
    Bars,
    Lines,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TimeMode {
    #[default]
    Elapsed,
    Remaining,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct WindowPos {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EqConfig {
    pub enabled: bool,
    pub auto: bool,
    pub preamp: f32,
    pub bands: [f32; 10],
}

impl Default for EqConfig {
    fn default() -> Self {
        Self { enabled: false, auto: false, preamp: 0.0, bands: [0.0; 10] }
    }
}

impl EqConfig {
    pub fn to_settings(&self) -> EqSettings {
        EqSettings { enabled: self.enabled, preamp_db: self.preamp, bands_db: self.bands }
    }
    pub fn from_settings(&mut self, s: &EqSettings) {
        self.enabled = s.enabled;
        self.preamp = s.preamp_db;
        self.bands = s.bands_db;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub name: String,
    pub preamp: f32,
    pub bands: [f32; 10],
}

impl Default for Preset {
    fn default() -> Self {
        Self { name: String::new(), preamp: 0.0, bands: [0.0; 10] }
    }
}

impl Preset {
    pub fn to_settings(&self) -> EqSettings {
        EqSettings { enabled: true, preamp_db: self.preamp, bands_db: self.bands }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub volume: f32,
    pub balance: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub eq: EqConfig,
    pub presets: Vec<Preset>,
    /// Per-file "auto" presets keyed by file name.
    pub auto_presets: Vec<Preset>,
    pub skin: Option<PathBuf>,
    pub double_size: bool,
    pub always_on_top: bool,
    pub time_mode: TimeMode,
    pub vis_mode: VisMode,
    pub analyzer_style: AnalyzerStyle,
    pub analyzer_peaks: bool,
    pub main_pos: Option<WindowPos>,
    pub eq_pos: Option<WindowPos>,
    pub pl_pos: Option<WindowPos>,
    pub main_shaded: bool,
    pub eq_shaded: bool,
    pub pl_shaded: bool,
    pub eq_visible: bool,
    pub pl_visible: bool,
    /// Playlist window size in tile units beyond the minimum.
    pub pl_extra_cols: u32,
    pub pl_extra_rows: u32,
    pub scroll_title: bool,
    pub snap_windows: bool,
    pub playlist_font_size: f32,
    pub read_tags_on_add: bool,
    pub show_bitrate_avg: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            volume: 0.8,
            balance: 0.0,
            shuffle: false,
            repeat: false,
            eq: EqConfig::default(),
            presets: Vec::new(),
            auto_presets: Vec::new(),
            skin: None,
            double_size: false,
            always_on_top: false,
            time_mode: TimeMode::Elapsed,
            vis_mode: VisMode::Analyzer,
            analyzer_style: AnalyzerStyle::Bars,
            analyzer_peaks: true,
            main_pos: None,
            eq_pos: None,
            pl_pos: None,
            main_shaded: false,
            eq_shaded: false,
            pl_shaded: false,
            eq_visible: true,
            pl_visible: true,
            pl_extra_cols: 0,
            pl_extra_rows: 0,
            scroll_title: true,
            snap_windows: true,
            playlist_font_size: 9.0,
            read_tags_on_add: true,
            show_bitrate_avg: true,
        }
    }
}

pub fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("org", "winamp-rust", "Winamp")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn skins_dir() -> PathBuf {
    config_dir().join("Skins")
}

pub fn config_path() -> PathBuf {
    config_dir().join("winamp.toml")
}

pub fn playlist_path() -> PathBuf {
    config_dir().join("winamp.m3u8")
}


impl Config {
    pub fn load() -> Config {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("config parse error ({}): {e}", path.display());
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::create_dir_all(skins_dir());
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    log::warn!("saving config: {e}");
                }
            }
            Err(e) => log::warn!("serialising config: {e}"),
        }
    }

    pub fn find_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn auto_preset_for(&self, path: &Path) -> Option<&Preset> {
        let key = path.file_name()?.to_string_lossy().to_string();
        self.auto_presets.iter().find(|p| p.name.eq_ignore_ascii_case(&key))
    }
}
