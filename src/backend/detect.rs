use super::sysfs;

// ---------------------------------------------------------------------------
// Sysfs path constants
// ---------------------------------------------------------------------------

/// Battery charge limit (write to set max charge percentage).
pub const CHARGE_CONTROL_END_THRESHOLD: &str =
    "/sys/class/power_supply/BAT0/charge_control_end_threshold";

/// Battery current charge percentage.
pub const BAT_CAPACITY: &str = "/sys/class/power_supply/BAT0/capacity";

/// Battery charging status (Charging / Discharging / Not charging / Full).
pub const BAT_STATUS: &str = "/sys/class/power_supply/BAT0/status";

/// Battery full charge capacity in microamp-hours.
pub const BAT_CHARGE_FULL: &str = "/sys/class/power_supply/BAT0/charge_full";

/// Battery design capacity in microamp-hours.
pub const BAT_CHARGE_FULL_DESIGN: &str = "/sys/class/power_supply/BAT0/charge_full_design";

pub const BAT_CYCLE_COUNT: &str = "/sys/class/power_supply/BAT0/cycle_count";
pub const BAT_VOLTAGE_NOW: &str = "/sys/class/power_supply/BAT0/voltage_now";
pub const BAT_CURRENT_NOW: &str = "/sys/class/power_supply/BAT0/current_now";

/// Fan / thermal policy (0=balanced, 1=performance, 2=quiet).
pub const THROTTLE_THERMAL_POLICY: &str =
    "/sys/devices/platform/asus-nb-wmi/throttle_thermal_policy";

/// Platform profile (low-power / balanced / performance).
pub const PLATFORM_PROFILE: &str = "/sys/firmware/acpi/platform_profile";

/// Keyboard backlight brightness (0-3).
pub const KBD_BACKLIGHT: &str = "/sys/class/leds/asus::kbd_backlight/brightness";

/// Maximum keyboard backlight brightness.
pub const KBD_BACKLIGHT_MAX: &str = "/sys/class/leds/asus::kbd_backlight/max_brightness";

/// DMI product name (laptop model).
pub const DMI_PRODUCT_NAME: &str = "/sys/class/dmi/id/product_name";

/// DMI BIOS version.
pub const DMI_BIOS_VERSION: &str = "/sys/class/dmi/id/bios_version";

/// DMI board vendor.
pub const DMI_BOARD_VENDOR: &str = "/sys/class/dmi/id/board_vendor";

// ---------------------------------------------------------------------------
// Hardware feature detection
// ---------------------------------------------------------------------------

/// Detected hardware features for this machine.
///
/// On startup the application probes sysfs to determine which controls are
/// available. Pages for unsupported features are hidden in the UI.
#[derive(Debug, Clone)]
pub struct HardwareFeatures {
    /// Battery charge limit control is available.
    pub battery: bool,
    /// Fan / thermal policy control is available.
    pub fan_profile: bool,
    /// Keyboard backlight control is available.
    pub keyboard_backlight: bool,
    /// Platform profile control is available.
    pub platform_profile: bool,
}

/// Probe sysfs paths and return a [`HardwareFeatures`] summary.
pub fn detect_features() -> HardwareFeatures {
    HardwareFeatures {
        battery: sysfs::exists(CHARGE_CONTROL_END_THRESHOLD)
            && sysfs::exists(BAT_CAPACITY),
        fan_profile: sysfs::exists(THROTTLE_THERMAL_POLICY),
        keyboard_backlight: sysfs::exists(KBD_BACKLIGHT),
        platform_profile: sysfs::exists(PLATFORM_PROFILE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_features_does_not_panic() {
        // On a non-ASUS machine every feature will be false -- that is fine.
        let features = detect_features();
        // Just verify the struct is constructed without error.
        let _ = format!("{features:?}");
    }
}
