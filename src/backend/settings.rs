use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::battery;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_charge_threshold")]
    pub charge_threshold: u8,
}

fn default_charge_threshold() -> u8 {
    battery::THRESHOLD_DEFAULT
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            charge_threshold: default_charge_threshold(),
        }
    }
}

fn settings_path() -> PathBuf {
    let config = dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"));
    config.join("myasus4linux").join("settings.toml")
}

pub fn load() -> Settings {
    let path = settings_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(settings)
        .expect("settings should always serialize");
    std::fs::write(&path, content)
}
