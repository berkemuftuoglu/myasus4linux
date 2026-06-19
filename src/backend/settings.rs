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
    // dirs_next resolves XDG_CONFIG_HOME / $HOME; only fall back if both are
    // unset, and even then to a real $HOME/.config -- never a literal "~".
    let config = dirs_next::config_dir().unwrap_or_else(|| {
        std::env::var_os("HOME").map_or_else(
            || PathBuf::from(".config"),
            |home| PathBuf::from(home).join(".config"),
        )
    });
    config.join("myasus4linux").join("settings.toml")
}

pub fn load() -> Settings {
    load_from(&settings_path())
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    save_to(&settings_path(), settings)
}

/// Read settings from a specific file, falling back to defaults when it is
/// missing or unparsable. Split out from [`load`] so it can be tested without
/// touching the real config directory.
fn load_from(path: &std::path::Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_to(path: &std::path::Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, content)
}

// The charge limit is restored across reboots by the myasusd daemon, which
// persists it on write and re-applies it at startup. No separate boot service
// and no pkexec escalation -- see crates/myasusd/src/helper.rs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_threshold_matches_battery_default() {
        assert_eq!(
            Settings::default().charge_threshold,
            battery::THRESHOLD_DEFAULT
        );
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        save_to(
            &path,
            &Settings {
                charge_threshold: 65,
            },
        )
        .unwrap();
        assert_eq!(load_from(&path).charge_threshold, 65);
    }

    #[test]
    fn save_creates_missing_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deeper/settings.toml");
        save_to(
            &path,
            &Settings {
                charge_threshold: 90,
            },
        )
        .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn load_missing_file_falls_back_to_default() {
        let settings = load_from(std::path::Path::new("/nonexistent/settings.toml"));
        assert_eq!(settings.charge_threshold, battery::THRESHOLD_DEFAULT);
    }

    #[test]
    fn load_garbage_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "this is not valid toml :::").unwrap();
        assert_eq!(
            load_from(&path).charge_threshold,
            battery::THRESHOLD_DEFAULT
        );
    }
}
