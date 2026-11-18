//! Settings and well-known paths.
//!
//! Settings live in a small JSON file next to the app config, not in the
//! database, because the database location itself is a setting. The file is
//! written atomically so a crash can never leave a half-written config.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::scoring::ScoreConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Auto,
    Dark,
    Light,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Auto => "Match system",
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CityQuality {
    High,
    #[default]
    Balanced,
    Low,
}

impl CityQuality {    pub fn label(self) -> &'static str {
        match self {
            CityQuality::High => "High",
            CityQuality::Balanced => "Balanced",
            CityQuality::Low => "Low",
        }
    }

    /// Target frame rate for the city view.
    pub fn fps(self) -> u32 {
        match self {
            CityQuality::High => 60,
            CityQuality::Balanced => 30,
            CityQuality::Low => 24,
        }
    }

    /// Offscreen render scale. Neon renders 1:1 and leans on 4x MSAA plus
    /// shader-level edge AA, which keeps the window grid sharp; Low trades
    /// sharpness for a smaller target.
    pub fn scale(self) -> f32 {
        match self {
            CityQuality::High | CityQuality::Balanced => 1.0,
            CityQuality::Low => 0.7,
        }
    }

    /// Cars and drones are the first thing to go when the machine is busy.
    pub fn traffic(self) -> bool {
        self != CityQuality::Low
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CityStyle {
    #[default]
    #[serde(alias = "Neon")]
    Daylight,
    #[serde(alias = "Synthwave")]
    Blueprint,
}

impl CityStyle {
    pub fn label(self) -> &'static str {
        match self {
            CityStyle::Daylight => "Daylight",
            CityStyle::Blueprint => "Blueprint",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MotionMode {
    #[default]
    Auto,
    Full,
    Reduced,
}

impl MotionMode {
    pub fn label(self) -> &'static str {
        match self {
            MotionMode::Auto => "Match system",
            MotionMode::Full => "Full",
            MotionMode::Reduced => "Reduced",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub motion: MotionMode,
    pub high_contrast: bool,
    /// Adds hatching to the timeline so focus levels are readable without colour.
    pub patterns: bool,
    pub text_scale: f32,
    /// No input for this long closes the session and is not scored.
    pub idle_timeout_secs: u32,
    pub goal_minutes: u32,
    pub break_reminder_minutes: u32,
    pub focus_mode: bool,
    pub start_on_boot: bool,
    pub start_minimized: bool,
    /// Empty means "use the default local app data folder".
    pub data_dir: String,
    pub onboarded: bool,
    pub city_quality: CityQuality,
    pub city_style: CityStyle,
    /// Quiet local drone while FocusMap runs. No files, no network.
    pub ambient_sound: bool,
    pub score: ScoreConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Auto,
            motion: MotionMode::Auto,
            high_contrast: false,
            patterns: false,
            text_scale: 1.0,
            idle_timeout_secs: 600,
            goal_minutes: 120,
            break_reminder_minutes: 90,
            focus_mode: false,
            start_on_boot: false,
            start_minimized: false,
            data_dir: String::new(),
            onboarded: false,
            city_quality: CityQuality::Balanced,
            city_style: CityStyle::Daylight,
            ambient_sound: false,
            score: ScoreConfig::default(),
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }

    pub fn data_dir(&self) -> PathBuf {
        if self.data_dir.trim().is_empty() {
            default_data_dir()
        } else {
            PathBuf::from(self.data_dir.trim())
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir().join("focusmap.db")
    }

    pub fn text_scale(&self) -> f32 {
        self.text_scale.clamp(0.8, 1.6)
    }
}

/// `%APPDATA%\FocusMap` (or the XDG equivalent).
pub fn config_dir() -> PathBuf {
    env_dir("APPDATA", ".config").join("FocusMap")
}

/// `%LOCALAPPDATA%\FocusMap` (or the XDG equivalent).
pub fn default_data_dir() -> PathBuf {
    env_dir("LOCALAPPDATA", ".local/share").join("FocusMap")
}

fn env_dir(var: &str, unix_fallback: &str) -> PathBuf {
    if let Some(dir) = std::env::var_os(var) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(unix_fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert!(s.goal_minutes > 0);
        assert!(s.idle_timeout_secs >= 60);
        assert!((s.text_scale() - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.city_quality, CityQuality::Balanced);
        assert_eq!(s.city_style, CityStyle::Daylight);
        assert!(!s.ambient_sound);
        assert!(!s.data_dir().as_os_str().is_empty());
    }

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        s.theme = ThemeMode::Light;
        s.goal_minutes = 200;
        s.score.deep_threshold = 80;
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn old_configs_gain_new_fields() {
        let back: Settings = serde_json::from_str(r#"{"goal_minutes":45}"#).unwrap();
        assert_eq!(back.goal_minutes, 45);
        assert_eq!(back.theme, ThemeMode::Auto);
    }

    #[test]
    fn retired_city_styles_map_to_the_new_pair() {
        let old: Settings = serde_json::from_str(r#"{"city_style":"Neon"}"#).unwrap();
        assert_eq!(old.city_style, CityStyle::Daylight);
        let synth: Settings = serde_json::from_str(r#"{"city_style":"Synthwave"}"#).unwrap();
        assert_eq!(synth.city_style, CityStyle::Blueprint);
    }
}
