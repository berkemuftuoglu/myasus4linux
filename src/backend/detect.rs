use super::sysfs;

// The three writable controls share their paths with the daemon; myasus-core
// owns the literals so the GUI and the daemon can never disagree on a path.
pub const CHARGE_CONTROL_END_THRESHOLD: &str = myasus_core::CHARGE_THRESHOLD_PATH;
pub const BAT_CAPACITY: &str = "/sys/class/power_supply/BAT0/capacity";
pub const BAT_STATUS: &str = "/sys/class/power_supply/BAT0/status";

/// Value in microamp-hours.
pub const BAT_CHARGE_FULL: &str = "/sys/class/power_supply/BAT0/charge_full";

/// Value in microamp-hours.
pub const BAT_CHARGE_FULL_DESIGN: &str = "/sys/class/power_supply/BAT0/charge_full_design";

/// Charge remaining, in microamp-hours.
pub const BAT_CHARGE_NOW: &str = "/sys/class/power_supply/BAT0/charge_now";

pub const BAT_CYCLE_COUNT: &str = "/sys/class/power_supply/BAT0/cycle_count";
pub const BAT_VOLTAGE_NOW: &str = "/sys/class/power_supply/BAT0/voltage_now";
pub const BAT_CURRENT_NOW: &str = "/sys/class/power_supply/BAT0/current_now";

pub const THROTTLE_THERMAL_POLICY: &str = myasus_core::FAN_PROFILE_PATH;

pub const KBD_BACKLIGHT: &str = myasus_core::KBD_BACKLIGHT_PATH;
pub const DMI_PRODUCT_NAME: &str = "/sys/class/dmi/id/product_name";
pub const DMI_BIOS_VERSION: &str = "/sys/class/dmi/id/bios_version";
pub const DMI_BOARD_VENDOR: &str = "/sys/class/dmi/id/board_vendor";

/// On startup the application probes sysfs to determine which controls are
/// available. Pages for unsupported features are hidden in the UI.
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per detected hardware capability; a bitfield would read worse"
)]
#[derive(Debug, Clone)]
pub struct HardwareFeatures {
    /// A battery is present (capacity and status are readable). Gates the whole
    /// battery dashboard, independent of whether the charge limit is adjustable.
    pub battery: bool,
    /// The charge-limit control exists, so the limit slider is meaningful.
    pub charge_limit: bool,
    pub fan_profile: bool,
    pub keyboard_backlight: bool,
}

/// Probe sysfs paths and return a [`HardwareFeatures`] summary.
pub fn detect_features() -> HardwareFeatures {
    HardwareFeatures {
        battery: sysfs::exists(BAT_CAPACITY) && sysfs::exists(BAT_STATUS),
        charge_limit: sysfs::exists(CHARGE_CONTROL_END_THRESHOLD),
        fan_profile: sysfs::exists(THROTTLE_THERMAL_POLICY),
        keyboard_backlight: sysfs::exists(KBD_BACKLIGHT),
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
