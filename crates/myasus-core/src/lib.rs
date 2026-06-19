//! Shared contract for the privileged operations the root helper performs.
//!
//! Paths, value ranges, and serialisation live here in exactly one place. The
//! daemon validates through this and writes the fixed path; a new privileged
//! feature adds a variant here rather than inventing another ad-hoc escalation
//! path. Nothing here trusts a caller-supplied path. This crate is GTK-free and
//! I/O-free so it stays 100% unit-testable -- the actual write lives in the
//! daemon.

pub const CHARGE_THRESHOLD_PATH: &str = "/sys/class/power_supply/BAT0/charge_control_end_threshold";
pub const FAN_PROFILE_PATH: &str = "/sys/devices/platform/asus-nb-wmi/throttle_thermal_policy";
pub const KBD_BACKLIGHT_PATH: &str = "/sys/class/leds/asus::kbd_backlight/brightness";

/// One privileged write. Carries its own value; the target path is fixed per
/// variant and never comes from the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    ChargeThreshold(u8),
    FanProfile(u8),
    KeyboardBacklight(u8),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidateError {
    #[error("charge threshold {0} out of range (40-100)")]
    ChargeThreshold(u8),
    #[error("fan profile {0} out of range (0-2)")]
    FanProfile(u8),
    #[error("keyboard brightness {0} out of range (0-3)")]
    KeyboardBacklight(u8),
}

impl Op {
    /// Range-check the value. The kernel also checks, but validating here means
    /// the daemon never even attempts an out-of-range write.
    pub fn validate(self) -> Result<(), ValidateError> {
        match self {
            Op::ChargeThreshold(v) if !(40..=100).contains(&v) => {
                Err(ValidateError::ChargeThreshold(v))
            }
            Op::FanProfile(v) if v > 2 => Err(ValidateError::FanProfile(v)),
            Op::KeyboardBacklight(v) if v > 3 => Err(ValidateError::KeyboardBacklight(v)),
            _ => Ok(()),
        }
    }

    /// The fixed sysfs path this operation writes. Never caller-influenced.
    pub fn path(self) -> &'static str {
        match self {
            Op::ChargeThreshold(_) => CHARGE_THRESHOLD_PATH,
            Op::FanProfile(_) => FAN_PROFILE_PATH,
            Op::KeyboardBacklight(_) => KBD_BACKLIGHT_PATH,
        }
    }

    pub fn raw_value(self) -> u8 {
        match self {
            Op::ChargeThreshold(v) | Op::FanProfile(v) | Op::KeyboardBacklight(v) => v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_threshold_range() {
        assert!(Op::ChargeThreshold(40).validate().is_ok());
        assert!(Op::ChargeThreshold(80).validate().is_ok());
        assert!(Op::ChargeThreshold(100).validate().is_ok());
        assert_eq!(
            Op::ChargeThreshold(39).validate(),
            Err(ValidateError::ChargeThreshold(39))
        );
        assert_eq!(
            Op::ChargeThreshold(101).validate(),
            Err(ValidateError::ChargeThreshold(101))
        );
    }

    #[test]
    fn fan_profile_range() {
        for v in 0..=2 {
            assert!(Op::FanProfile(v).validate().is_ok());
        }
        assert_eq!(
            Op::FanProfile(3).validate(),
            Err(ValidateError::FanProfile(3))
        );
    }

    #[test]
    fn keyboard_range() {
        for v in 0..=3 {
            assert!(Op::KeyboardBacklight(v).validate().is_ok());
        }
        assert_eq!(
            Op::KeyboardBacklight(4).validate(),
            Err(ValidateError::KeyboardBacklight(4))
        );
    }

    #[test]
    fn paths_are_fixed_per_variant() {
        assert_eq!(Op::ChargeThreshold(80).path(), CHARGE_THRESHOLD_PATH);
        assert_eq!(Op::FanProfile(1).path(), FAN_PROFILE_PATH);
        assert_eq!(Op::KeyboardBacklight(2).path(), KBD_BACKLIGHT_PATH);
    }

    #[test]
    fn raw_value_unwraps_inner() {
        assert_eq!(Op::ChargeThreshold(80).raw_value(), 80);
        assert_eq!(Op::FanProfile(2).raw_value(), 2);
        assert_eq!(Op::KeyboardBacklight(3).raw_value(), 3);
    }
}
