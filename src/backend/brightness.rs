//! Screen (display) backlight control via /sys/class/backlight.

use std::path::PathBuf;

use super::error::BackendError;
use super::sysfs;

fn backlight_dir() -> Option<PathBuf> {
    let root = std::path::Path::new("/sys/class/backlight");
    let mut fallback = None;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some("intel_backlight") {
            return Some(path);
        }
        fallback.get_or_insert(path);
    }
    fallback
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
    let max: u32 = sysfs::read_value(dir.join("max_brightness").to_str().unwrap_or_default())?;
    let value = to_raw(percent, max).to_string();
    let Some(path) = dir.join("brightness").to_str().map(ToOwned::to_owned) else {
        return Ok(());
    };
    sysfs::write(&path, &value).or_else(|_| sysfs::write_privileged(&path, &value))
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
