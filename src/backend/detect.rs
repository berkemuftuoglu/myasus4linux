use super::sysfs;

// The writable controls share their paths with the daemon; myasus-core owns the
// literals so the GUI and the daemon can never disagree. The battery is resolved
// at runtime (see `battery::battery_dir`) because it is not always `BAT0`.
pub const THROTTLE_THERMAL_POLICY: &str = myasus_core::FAN_PROFILE_PATH;

pub const KBD_BACKLIGHT: &str = myasus_core::KBD_BACKLIGHT_PATH;
pub const DMI_PRODUCT_NAME: &str = "/sys/class/dmi/id/product_name";
pub const DMI_PRODUCT_FAMILY: &str = "/sys/class/dmi/id/product_family";
pub const DMI_BIOS_VERSION: &str = "/sys/class/dmi/id/bios_version";
pub const DMI_BOARD_VENDOR: &str = "/sys/class/dmi/id/board_vendor";
/// Per-model identifier; the natural key for any future model-specific quirks.
pub const DMI_BOARD_NAME: &str = "/sys/class/dmi/id/board_name";

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
    let battery_dir = super::battery::battery_dir();
    HardwareFeatures {
        battery: battery_dir
            .as_ref()
            .is_some_and(|d| d.join("capacity").exists() && d.join("status").exists()),
        charge_limit: battery_dir
            .as_ref()
            .is_some_and(|d| d.join(myasus_core::CHARGE_THRESHOLD_ATTR).exists()),
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
