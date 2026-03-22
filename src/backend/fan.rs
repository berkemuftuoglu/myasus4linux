use super::detect;
use super::error::BackendError;
use super::sysfs;

/// ASUS fan / thermal profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanProfile {
    /// Balanced mode (default).
    Balanced = 0,
    /// Performance mode (fans spin faster, higher TDP).
    Performance = 1,
    /// Quiet mode (fans spin slower, lower TDP).
    Quiet = 2,
}

impl FanProfile {
    pub fn from_raw(value: u8) -> Result<Self, BackendError> {
        match value {
            0 => Ok(Self::Balanced),
            1 => Ok(Self::Performance),
            2 => Ok(Self::Quiet),
            other => Err(BackendError::UnknownFanProfile(other)),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::Performance => "Performance",
            Self::Quiet => "Quiet",
        }
    }
}

pub fn read_profile() -> Result<FanProfile, BackendError> {
    let raw: u8 = sysfs::read_value(detect::THROTTLE_THERMAL_POLICY)?;
    FanProfile::from_raw(raw)
}

/// Find the CPU thermal zone and return its temp in degrees C.
/// Scans /sys/class/thermal/thermal_zone*/type for known CPU zone names.
pub fn read_cpu_temp() -> Option<f64> {
    let thermal = std::path::Path::new("/sys/class/thermal");
    let cpu_zone_names = ["x86_pkg_temp", "TCPU", "acpitz", "coretemp"];

    let entries = std::fs::read_dir(thermal).ok()?;
    for entry in entries.flatten() {
        let zone = entry.path();
        let zone_type = std::fs::read_to_string(zone.join("type")).ok()?;
        if cpu_zone_names.iter().any(|name| zone_type.trim() == *name) {
            let raw = std::fs::read_to_string(zone.join("temp")).ok()?;
            let millideg: f64 = raw.trim().parse().ok()?;
            return Some(millideg / 1000.0);
        }
    }
    None
}

pub fn set_profile(profile: FanProfile) -> Result<(), BackendError> {
    sysfs::write_privileged(
        detect::THROTTLE_THERMAL_POLICY,
        &(profile as u8).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_profile_roundtrip() {
        for raw in 0..=2u8 {
            let profile = FanProfile::from_raw(raw).expect("valid profile");
            assert_eq!(profile as u8, raw);
        }
    }

    #[test]
    fn fan_profile_rejects_invalid() {
        assert!(FanProfile::from_raw(3).is_err());
        assert!(FanProfile::from_raw(255).is_err());
    }

    #[test]
    fn fan_profile_labels() {
        assert_eq!(FanProfile::Balanced.label(), "Balanced");
        assert_eq!(FanProfile::Performance.label(), "Performance");
        assert_eq!(FanProfile::Quiet.label(), "Quiet");
    }
}
