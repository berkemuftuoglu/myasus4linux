//! Screen (display) backlight control via /sys/class/backlight.

use std::path::PathBuf;

use super::error::BackendError;
use super::sysfs;

/// Most laptops expose one panel backlight, but some show several (e.g. an AMD
/// `amdgpu_bl*` plus a firmware `acpi_video0`). Pick the most specific real
/// controller in preference order; only then fall back to the first by name so
/// the choice is deterministic across boots rather than `read_dir` order.
fn backlight_dir() -> Option<PathBuf> {
    const PREFERRED: [&str; 4] = ["intel_backlight", "amdgpu_bl1", "amdgpu_bl0", "acpi_video0"];
    let root = std::path::Path::new("/sys/class/backlight");
    let mut devices: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    devices.sort();
    for name in PREFERRED {
        if let Some(p) = devices
            .iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(name))
        {
            return Some(p.clone());
        }
    }
    devices.into_iter().next()
}

pub fn available() -> bool {
    backlight_dir().is_some()
}

/// Current screen brightness as a percentage, if a backlight exists.
pub fn read_percent() -> Option<u8> {
    let dir = backlight_dir()?;
    let cur: u32 = sysfs::read_value(dir.join("brightness").to_str()?).ok()?;
    let max: u32 = sysfs::read_value(dir.join("max_brightness").to_str()?).ok()?;
    Some(to_percent(cur, max))
}

pub fn set_percent(percent: u8) -> Result<(), BackendError> {
    let dir = backlight_dir().ok_or_else(|| BackendError::SysfsWrite {
        path: "/sys/class/backlight".to_owned(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "no backlight device"),
    })?;
    let max_path = dir.join("max_brightness");
    let max: u32 = sysfs::read_value(non_utf8_guard(&max_path)?)?;
    let value = to_raw(percent, max).to_string();
    let brightness_path = dir.join("brightness");
    // The screen backlight is made writable for the active session by a logind
    // uaccess udev rule (data/99-myasus4linux-backlight.rules), the same way
    // GNOME dims the screen. No privilege escalation needed.
    sysfs::write(non_utf8_guard(&brightness_path)?, &value)
}

/// sysfs paths are ASCII in practice; if one somehow isn't valid UTF-8 we
/// surface it as an error rather than silently reporting success.
fn non_utf8_guard(path: &std::path::Path) -> Result<&str, BackendError> {
    path.to_str().ok_or_else(|| BackendError::SysfsWrite {
        path: path.to_string_lossy().into_owned(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "backlight path is not valid UTF-8",
        ),
    })
}

fn to_percent(cur: u32, max: u32) -> u8 {
    if max == 0 {
        return 0;
    }
    u8::try_from(u64::from(cur) * 100 / u64::from(max)).unwrap_or(100)
}

/// Never below ~5% so the screen can't go fully dark by accident.
fn to_raw(percent: u8, max: u32) -> u32 {
    let floor = (max / 20).max(1);
    (u32::from(percent.min(100)) * max / 100).max(floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_round_trips() {
        assert_eq!(to_percent(4800, 9600), 50);
        assert_eq!(to_percent(9600, 9600), 100);
        assert_eq!(to_percent(0, 9600), 0);
    }

    #[test]
    fn raw_respects_floor_and_ceiling() {
        assert_eq!(to_raw(100, 10000), 10000);
        assert_eq!(to_raw(50, 10000), 5000);
        // 0% clamps up to the 5% floor, not full dark
        assert_eq!(to_raw(0, 10000), 500);
    }
}
