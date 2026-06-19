use std::path::{Path, PathBuf};

use super::error::BackendError;

/// The keyboard backlight `brightness` attribute, resolved at runtime (the LED's
/// sysname is vendor-prefixed and varies), falling back to the canonical literal.
fn kbd_path() -> PathBuf {
    myasus_core::kbd_backlight_path(Path::new(myasus_core::LEDS_ROOT))
        .unwrap_or_else(|| PathBuf::from(myasus_core::KBD_BACKLIGHT_PATH))
}

pub fn read_brightness() -> Result<u8, BackendError> {
    let path = kbd_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| BackendError::SysfsRead {
            path: path.display().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "keyboard backlight"),
        })
}

pub fn set_brightness(value: u8) -> Result<(), BackendError> {
    myasus_core::Op::KeyboardBacklight(value).validate()?;
    super::ipc::set_keyboard_backlight(value)
}

pub fn brightness_label(value: u8) -> &'static str {
    match value {
        0 => "Off",
        1 => "Low",
        2 => "Medium",
        3 => "High",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_rejects_above_3() {
        let err = set_brightness(4).unwrap_err();
        assert!(matches!(
            err,
            BackendError::Validate(myasus_core::ValidateError::KeyboardBacklight(4))
        ));
    }

    #[test]
    fn brightness_labels() {
        assert_eq!(brightness_label(0), "Off");
        assert_eq!(brightness_label(1), "Low");
        assert_eq!(brightness_label(2), "Medium");
        assert_eq!(brightness_label(3), "High");
        assert_eq!(brightness_label(99), "Unknown");
    }
}
