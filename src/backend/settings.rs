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
    let config = dirs_next::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
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
    let content = toml::to_string_pretty(settings).expect("settings should always serialize");
    std::fs::write(&path, content)
}

const SERVICE_NAME: &str = "myasus4linux-charge-limit.service";
const SERVICE_PATH: &str = "/etc/systemd/system/myasus4linux-charge-limit.service";

pub fn boot_service_installed() -> bool {
    std::path::Path::new(SERVICE_PATH).exists()
}

/// Installs and enables the systemd service that restores the charge limit on boot.
/// Uses pkexec so the user gets one password prompt.
pub fn install_boot_service() -> Result<(), String> {
    let service_content = include_str!("../../data/systemd/myasus4linux-charge-limit.service");

    // Write service file via pkexec tee
    let mut child = std::process::Command::new("pkexec")
        .args(["tee", SERVICE_PATH])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        stdin
            .write_all(service_content.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("failed to write service file".to_owned());
    }

    // Enable the service
    let enable = std::process::Command::new("pkexec")
        .args(["systemctl", "enable", SERVICE_NAME])
        .status()
        .map_err(|e| e.to_string())?;

    if enable.success() {
        Ok(())
    } else {
        Err("failed to enable service".to_owned())
    }
}
