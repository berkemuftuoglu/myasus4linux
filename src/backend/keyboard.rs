use super::detect;
use super::error::BackendError;
use super::sysfs;

pub fn read_brightness() -> Result<u8, BackendError> {
    sysfs::read_value(detect::KBD_BACKLIGHT)
}

pub fn set_brightness(value: u8) -> Result<(), BackendError> {
    myasus_core::Op::KeyboardBacklight(value).validate()?;
    super::daemon::set_keyboard_backlight(value)
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
