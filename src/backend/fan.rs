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
    /// Convert a raw sysfs integer to a `FanProfile`.
    pub fn from_raw(value: u8) -> Result<Self, BackendError> {
        match value {
            0 => Ok(Self::Balanced),
            1 => Ok(Self::Performance),
            2 => Ok(Self::Quiet),
            other => Err(BackendError::UnknownFanProfile(other)),
        }
    }

    /// Return the raw sysfs integer for this profile.
    pub fn as_raw(self) -> u8 {
        self as u8
    }

    /// Human-readable label for the profile.
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::Performance => "Performance",
            Self::Quiet => "Quiet",
        }
    }
}

impl std::fmt::Display for FanProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Read the current fan profile from sysfs.
pub fn read_profile() -> Result<FanProfile, BackendError> {
    let raw: u8 = sysfs::read_value(detect::THROTTLE_THERMAL_POLICY)?;
    FanProfile::from_raw(raw)
}

/// Set the fan profile via privileged write.
pub fn set_profile(profile: FanProfile) -> Result<(), BackendError> {
    sysfs::write_privileged(
        detect::THROTTLE_THERMAL_POLICY,
        &profile.as_raw().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_profile_roundtrip() {
        for raw in 0..=2u8 {
            let profile = FanProfile::from_raw(raw).expect("valid profile");
            assert_eq!(profile.as_raw(), raw);
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
