//! Persistent settings (the equivalent of winamp.ini) stored as TOML.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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
        Self {
            enabled: false,
            auto: false,
            preamp: 0.0,
            bands: [0.0; 10],
        }
    }
}

impl EqConfig {
    pub fn to_settings(&self) -> EqSettings {
        EqSettings {
            enabled: self.enabled,
            preamp_db: self.preamp,
            bands_db: self.bands,
        }
    }
    /// Remember the equalizer state the player currently has.
    pub fn update_from(&mut self, settings: &EqSettings) {
        self.enabled = settings.enabled;
        self.preamp = settings.preamp_db;
        self.bands = settings.bands_db;
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
        Self {
            name: String::new(),
            preamp: 0.0,
            bands: [0.0; 10],
        }
    }
}

impl Preset {
    pub fn from_settings(name: impl Into<String>, settings: &EqSettings) -> Self {
        Self {
            name: name.into(),
            preamp: settings.preamp_db,
            bands: settings.bands_db,
        }
    }

    /// Presets are always applied with the equalizer switched on.
    pub fn to_settings(&self) -> EqSettings {
        EqSettings {
            enabled: true,
            preamp_db: self.preamp,
            bands_db: self.bands,
        }
    }

    pub fn has_name(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
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
    pub lock_windows: bool,
    pub milkdrop_visible: bool,
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
            lock_windows: true,
            milkdrop_visible: false,
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
    /// Load the settings file, falling back to defaults when it is missing
    /// or unreadable (a broken file is logged, not fatal).
    pub fn load() -> Config {
        Self::load_from(&config_path())
    }

    fn load_from(path: &Path) -> Config {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Config::default();
        };
        toml::from_str(&text).unwrap_or_else(|e| {
            log::warn!("config parse error ({}): {e}", path.display());
            Config::default()
        })
    }

    /// Write the settings file, creating the config and skins folders.
    pub fn save(&self) -> Result<()> {
        if let Err(e) = std::fs::create_dir_all(skins_dir()) {
            log::warn!("creating skins folder: {e}");
        }
        self.save_to(&config_path())
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serialising config")?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }

    pub fn find_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|preset| preset.has_name(name))
    }

    /// The per-file auto preset for `path`, matched on file name.
    pub fn auto_preset_for(&self, path: &Path) -> Option<&Preset> {
        let key = path.file_name()?.to_string_lossy();
        self.auto_presets
            .iter()
            .find(|preset| preset.has_name(&key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("winamp-config-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = temp_file("roundtrip/winamp.toml");
        let mut config = Config {
            volume: 0.42,
            shuffle: true,
            pl_extra_rows: 3,
            ..Config::default()
        };
        config.presets.push(Preset {
            name: "Mine".into(),
            preamp: 1.5,
            bands: [2.0; 10],
        });
        config.skin = Some(PathBuf::from("/skins/x.wsz"));
        config.save_to(&path).unwrap();
        let loaded = Config::load_from(&path);
        assert_eq!(loaded.volume, 0.42);
        assert!(loaded.shuffle);
        assert_eq!(loaded.pl_extra_rows, 3);
        assert_eq!(loaded.presets[0].name, "Mine");
        assert_eq!(loaded.skin.as_deref(), Some(Path::new("/skins/x.wsz")));
    }

    #[test]
    fn missing_or_partial_files_fall_back_to_defaults() {
        assert_eq!(
            Config::load_from(Path::new("/no/such/winamp.toml")).volume,
            Config::default().volume
        );
        let path = temp_file("partial.toml");
        std::fs::write(
            &path,
            "volume = 0.1
[eq]
enabled = true
",
        )
        .unwrap();
        let loaded = Config::load_from(&path);
        assert_eq!(loaded.volume, 0.1);
        assert!(loaded.eq.enabled);
        assert!(
            loaded.snap_windows,
            "unspecified fields keep their defaults"
        );
        std::fs::write(&path, "this is = not [ toml").unwrap();
        assert_eq!(
            Config::load_from(&path).volume,
            Config::default().volume,
            "garbage is not fatal"
        );
    }

    #[test]
    fn presets_convert_to_and_from_player_settings() {
        let settings = EqSettings {
            enabled: false,
            preamp_db: -3.0,
            bands_db: [1.0; 10],
        };
        let preset = Preset::from_settings("Rock", &settings);
        assert_eq!(preset.name, "Rock");
        assert!(
            preset.to_settings().enabled,
            "applying a preset switches the EQ on"
        );
        assert_eq!(preset.to_settings().preamp_db, -3.0);
        let mut eq_config = EqConfig::default();
        eq_config.update_from(&settings);
        assert_eq!(eq_config.to_settings(), settings);
    }

    #[test]
    fn preset_lookup_ignores_case_and_matches_file_names() {
        let config = Config {
            presets: vec![Preset {
                name: "Dance".into(),
                ..Preset::default()
            }],
            auto_presets: vec![Preset {
                name: "Song.MP3".into(),
                ..Preset::default()
            }],
            ..Config::default()
        };
        assert!(config.find_preset("dance").is_some());
        assert!(config.find_preset("rock").is_none());
        assert!(config
            .auto_preset_for(Path::new("/music/song.mp3"))
            .is_some());
        assert!(config
            .auto_preset_for(Path::new("/music/other.mp3"))
            .is_none());
    }
}
