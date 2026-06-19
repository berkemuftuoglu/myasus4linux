// Panics/indexing are fine in tests; the panic-class lints only guard production.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

//! Shared contract for the privileged operations the root helper performs.
//!
//! Paths, value ranges, and serialisation live here in exactly one place. The
//! daemon validates through this and writes the fixed path; a new privileged
//! feature adds a variant here rather than inventing another ad-hoc escalation
//! path. Nothing here trusts a caller-supplied path. This crate is GTK-free;
//! the only I/O is the battery-directory resolver, which takes an injectable
//! root so it stays temp-dir testable.

use std::path::{Path, PathBuf};

/// Root under which the kernel exposes batteries and AC adapters. The battery is
/// NOT always `BAT0` (real non-ROG laptops use `BAT1`/`BATT`/`BATC`), so the
/// device is resolved at runtime by [`battery_dir`] rather than hardcoded.
pub const POWER_SUPPLY_ROOT: &str = "/sys/class/power_supply";
/// The charge-limit control, relative to the resolved battery directory.
pub const CHARGE_THRESHOLD_ATTR: &str = "charge_control_end_threshold";
pub const FAN_PROFILE_PATH: &str = "/sys/devices/platform/asus-nb-wmi/throttle_thermal_policy";
/// Kernel-standard ACPI performance interface, the fallback when the ASUS WMI
/// `throttle_thermal_policy` is absent (more non-ROG laptops expose this one).
pub const PLATFORM_PROFILE_PATH: &str = "/sys/firmware/acpi/platform_profile";
/// Space-separated tokens this machine's `platform_profile` accepts.
pub const PLATFORM_PROFILE_CHOICES_PATH: &str = "/sys/firmware/acpi/platform_profile_choices";
pub const KBD_BACKLIGHT_PATH: &str = "/sys/class/leds/asus::kbd_backlight/brightness";
/// Root under which the kernel exposes LEDs. The keyboard backlight's sysname is
/// vendor-prefixed and varies, so it is matched at runtime by [`kbd_backlight_path`].
pub const LEDS_ROOT: &str = "/sys/class/leds";

/// The well-known D-Bus name the privileged helper owns on the system bus, and
/// the object path it serves. The daemon connects/serves with these; the client
/// proxy targets them. NOTE: `#[zbus::interface]`/`#[zbus::proxy]` attribute
/// strings cannot reference a const, so the literals there must stay identical
/// to these.
pub const DBUS_NAME: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper";
pub const DBUS_PATH: &str = "/io/github/berkmuftuoglu/MyAsus4Linux/Helper";

/// Charge limit bounds. Below `CHARGE_MIN` the battery can over-discharge; the
/// kernel rejects anything outside this, but owning the range here means the
/// daemon and the GUI agree on one definition instead of three.
pub const CHARGE_MIN: u8 = 40;
pub const CHARGE_MAX: u8 = 100;

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

    /// The fixed sysfs path for operations that have one, or `None` for those
    /// whose device must be resolved at runtime. Only [`Op::ChargeThreshold`] is
    /// runtime-resolved (the battery enumerates as BAT0/BAT1/...); the daemon
    /// builds its path with [`charge_threshold_path`]. Never caller-influenced.
    pub fn fixed_path(self) -> Option<&'static str> {
        match self {
            Op::ChargeThreshold(_) => None,
            Op::FanProfile(_) => Some(FAN_PROFILE_PATH),
            Op::KeyboardBacklight(_) => Some(KBD_BACKLIGHT_PATH),
        }
    }

    pub fn raw_value(self) -> u8 {
        match self {
            Op::ChargeThreshold(v) | Op::FanProfile(v) | Op::KeyboardBacklight(v) => v,
        }
    }
}

/// Resolve the main battery's sysfs directory under `root` (normally
/// [`POWER_SUPPLY_ROOT`]). Follows asusctl's cascade: prefer the device exposing
/// the charge-threshold control, else a sysname starting `BAT`, else any
/// `type == Battery`. When several qualify, the largest design capacity wins so
/// a tiny secondary cell or UPS can't shadow the main pack. `None` on a desktop
/// with no battery.
pub fn battery_dir(root: &Path) -> Option<PathBuf> {
    let batteries: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_battery(p))
        .collect();

    let with_attr: Vec<PathBuf> = batteries
        .iter()
        .filter(|p| p.join(CHARGE_THRESHOLD_ATTR).exists())
        .cloned()
        .collect();
    if let Some(best) = largest_capacity(&with_attr) {
        return Some(best);
    }

    let named: Vec<PathBuf> = batteries
        .iter()
        .filter(|p| sysname_starts_with_bat(p))
        .cloned()
        .collect();
    if let Some(best) = largest_capacity(&named) {
        return Some(best);
    }

    largest_capacity(&batteries)
}

/// Full path to the charge-limit control on the resolved battery, if present.
pub fn charge_threshold_path(root: &Path) -> Option<PathBuf> {
    Some(battery_dir(root)?.join(CHARGE_THRESHOLD_ATTR))
}

/// Resolve the keyboard backlight `brightness` attribute under `root` (normally
/// [`LEDS_ROOT`]). Matches any LED whose sysname contains `kbd_backlight`, since
/// the vendor prefix varies (`asus::kbd_backlight`, ...). `None` if absent.
pub fn kbd_backlight_path(root: &Path) -> Option<PathBuf> {
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("kbd_backlight"))
        })
        .map(|p| p.join("brightness"))
}

/// The `platform_profile` token for a canonical fan-profile value (0=balanced,
/// 1=performance, 2=quiet), or `None` if out of range.
pub fn platform_profile_token(value: u8) -> Option<&'static str> {
    match value {
        0 => Some("balanced"),
        1 => Some("performance"),
        2 => Some("quiet"),
        _ => None,
    }
}

fn is_battery(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("type")).is_ok_and(|t| t.trim().eq_ignore_ascii_case("Battery"))
}

fn sysname_starts_with_bat(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_uppercase().starts_with("BAT"))
}

/// Design capacity as a comparable magnitude (energy uWh if present, else charge
/// uAh) used only to rank candidate batteries, never as an absolute value.
fn design_capacity(dir: &Path) -> u64 {
    read_u64(dir, "energy_full_design")
        .or_else(|| read_u64(dir, "charge_full_design"))
        .unwrap_or(0)
}

fn read_u64(dir: &Path, attr: &str) -> Option<u64> {
    std::fs::read_to_string(dir.join(attr)).ok()?.trim().parse().ok()
}

fn largest_capacity(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().max_by_key(|p| design_capacity(p)).cloned()
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
    fn fixed_paths_per_variant() {
        // The charge threshold is resolved at runtime (battery enumerates), so
        // it has no fixed path; the other two are fixed.
        assert_eq!(Op::ChargeThreshold(80).fixed_path(), None);
        assert_eq!(Op::FanProfile(1).fixed_path(), Some(FAN_PROFILE_PATH));
        assert_eq!(Op::KeyboardBacklight(2).fixed_path(), Some(KBD_BACKLIGHT_PATH));
    }

    #[test]
    fn fixed_paths_are_absolute_sys_attributes() {
        for op in [Op::FanProfile(1), Op::KeyboardBacklight(2)] {
            let path = op.fixed_path().unwrap();
            assert!(path.starts_with("/sys/"), "{path} escapes /sys");
            assert!(Path::new(path).is_absolute());
        }
    }

    #[test]
    fn battery_dir_prefers_threshold_then_bat_then_largest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // BAT0: a battery but no charge-threshold control.
        mk_battery(root, "BAT0", Some(50_000_000), None);
        // BAT1: exposes the threshold control -> wins tier 1.
        mk_battery(root, "BAT1", Some(60_000_000), Some(80));
        assert_eq!(battery_dir(root), Some(root.join("BAT1")));
    }

    #[test]
    fn battery_dir_skips_tiny_secondary_cell() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A tiny UPS-like cell with the control, and the real main pack with it.
        mk_battery(root, "BATT", Some(5_000_000), Some(80));
        mk_battery(root, "BAT0", Some(62_000_000), Some(80));
        assert_eq!(battery_dir(root), Some(root.join("BAT0")));
    }

    #[test]
    fn battery_dir_none_without_a_battery() {
        let dir = tempfile::tempdir().unwrap();
        // An AC adapter is present but no battery (desktop).
        let ac = dir.path().join("AC0");
        std::fs::create_dir_all(&ac).unwrap();
        std::fs::write(ac.join("type"), "Mains\n").unwrap();
        assert_eq!(battery_dir(dir.path()), None);
    }

    #[test]
    fn charge_threshold_path_joins_resolved_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        mk_battery(root, "BATC", Some(50_000_000), Some(80));
        assert_eq!(
            charge_threshold_path(root),
            Some(root.join("BATC").join(CHARGE_THRESHOLD_ATTR))
        );
    }

    #[test]
    fn kbd_backlight_matches_vendor_prefixed_led() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("input3::capslock")).unwrap();
        let kbd = root.join("asus::kbd_backlight");
        std::fs::create_dir_all(&kbd).unwrap();
        assert_eq!(kbd_backlight_path(root), Some(kbd.join("brightness")));
    }

    #[test]
    fn kbd_backlight_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("input3::scrolllock")).unwrap();
        assert_eq!(kbd_backlight_path(dir.path()), None);
    }

    fn mk_battery(root: &Path, name: &str, energy_full_design: Option<u64>, threshold: Option<u8>) {
        let d = root.join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("type"), "Battery\n").unwrap();
        if let Some(e) = energy_full_design {
            std::fs::write(d.join("energy_full_design"), format!("{e}\n")).unwrap();
        }
        if let Some(t) = threshold {
            std::fs::write(d.join(CHARGE_THRESHOLD_ATTR), format!("{t}\n")).unwrap();
        }
    }

    #[test]
    fn raw_value_unwraps_inner() {
        assert_eq!(Op::ChargeThreshold(80).raw_value(), 80);
        assert_eq!(Op::FanProfile(2).raw_value(), 2);
        assert_eq!(Op::KeyboardBacklight(3).raw_value(), 3);
    }
}
