//! Persisted user configuration for AsBar.
//!
//! Lives at `C:/AsBar/config.json`. The island geometry is applied by the Rust
//! side (window size/position), while colors/opacity/radius are forwarded to the
//! webview and applied as CSS custom properties.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Root directory that hosts the config file and the icon cache.
pub fn root_dir() -> PathBuf {
    PathBuf::from("C:/AsBar")
}

pub fn icons_dir() -> PathBuf {
    root_dir().join("Assets").join("Icons")
}

fn config_path() -> PathBuf {
    root_dir().join("config.json")
}

/// Where on the screen the island docks horizontally.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    Left,
    Center,
    Right,
}

impl Default for Anchor {
    fn default() -> Self {
        Anchor::Center
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Collapsed island width in logical pixels.
    pub width: u32,
    /// Collapsed island height in logical pixels.
    pub height: u32,
    /// Distance from the top edge of the work area, logical pixels.
    pub margin_top: i32,
    /// Horizontal anchor on the primary monitor.
    pub anchor: Anchor,
    /// Manual horizontal nudge applied after anchoring, logical pixels.
    pub offset_x: i32,
    /// Background color of the island (hex, e.g. `#0B0B0F`).
    pub bg_color: String,
    /// Primary text color (hex).
    pub text_color: String,
    /// Accent color used for the progress bar / active controls (hex).
    pub accent_color: String,
    /// Overall island opacity, 0.0–1.0.
    pub opacity: f64,
    /// Corner radius in logical pixels.
    pub corner_radius: u32,
    /// When true, `accent_color` is overridden by the Windows system accent.
    pub follow_system_accent: bool,
    /// Keep the island above other windows.
    pub always_on_top: bool,
    /// When true, the island tints itself with the album-art dominant color.
    pub dynamic_accent: bool,
    /// Whether the launch-on-startup shortcut is installed.
    pub autostart: bool,
    /// UI language: `"ru"` or `"en"`.
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            width: 510,
            height: 100,
            margin_top: 0,
            anchor: Anchor::Center,
            offset_x: 0,
            bg_color: "#050507".into(),
            text_color: "#FFFFFF".into(),
            accent_color: "#E0E0EC".into(),
            opacity: 0.9,
            corner_radius: 30,
            follow_system_accent: false,
            always_on_top: true,
            dynamic_accent: true,
            autostart: false,
            language: "ru".into(),
        }
    }
}

impl Config {
    /// Load the config from disk, falling back to defaults on any error.
    pub fn load() -> Config {
        match std::fs::read_to_string(config_path()) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    /// Persist the config to disk, creating the AsBar tree if needed.
    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(root_dir())?;
        std::fs::create_dir_all(icons_dir())?;
        let raw = serde_json::to_string_pretty(self).expect("config serializes");
        std::fs::write(config_path(), raw)
    }
}

/// Ensure the AsBar directory tree exists. Cheap to call repeatedly.
pub fn ensure_dirs() {
    let _ = std::fs::create_dir_all(icons_dir());
}
