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

    /// The kernel's numeric encoding for this profile, written by the daemon.
    pub fn as_raw(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::Performance => "Performance",
            Self::Quiet => "Quiet",
        }
    }

    /// Map a `platform_profile` token to a profile via the shared core mapping.
    /// `low-power` collapses into `Quiet` (the closest of our three buckets).
    pub fn from_platform_token(token: &str) -> Result<Self, BackendError> {
        myasus_core::profile_from_token(token)
            .and_then(|v| Self::from_raw(v).ok())
            .ok_or_else(|| BackendError::ParseError {
                path: myasus_core::PLATFORM_PROFILE_PATH.to_owned(),
                details: format!("unknown platform_profile {token:?}"),
            })
    }
}

/// Read the active profile from the ASUS WMI interface if present, else from the
/// kernel-standard `platform_profile`.
pub fn read_profile() -> Result<FanProfile, BackendError> {
    if sysfs::exists(detect::THROTTLE_THERMAL_POLICY) {
        let raw: u8 = sysfs::read_value(detect::THROTTLE_THERMAL_POLICY)?;
        return FanProfile::from_raw(raw);
    }
    FanProfile::from_platform_token(&sysfs::read(myasus_core::PLATFORM_PROFILE_PATH)?)
}

/// The CPU temperature in degrees C, via the shared core reader (package/core
/// sensors preferred over the generic ACPI zone; implausible sentinels dropped).
pub fn read_cpu_temp() -> Option<f64> {
    myasus_core::cpu_temp(std::path::Path::new(myasus_core::THERMAL_ROOT))
}

pub fn set_profile(profile: FanProfile) -> Result<(), BackendError> {
    super::ipc::set_fan_profile(profile.as_raw())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanReading {
    pub label: String,
    pub rpm: u32,
}

/// Read all fan tachometers the kernel exposes under hwmon.
/// Many ASUS laptops expose none, in which case this is empty and the UI should
/// say so rather than invent a number.
pub fn read_fans() -> Vec<FanReading> {
    scan_fans(std::path::Path::new("/sys/class/hwmon"))
}

fn scan_fans(hwmon_root: &std::path::Path) -> Vec<FanReading> {
    let mut fans = Vec::new();
    let Ok(chips) = std::fs::read_dir(hwmon_root) else {
        return fans;
    };
    for chip in chips.flatten() {
        let dir = chip.path();
        let chip_name = std::fs::read_to_string(dir.join("name"))
            .ok()
            .map(|s| s.trim().to_owned());

        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut indices: Vec<u32> = files
            .flatten()
            .filter_map(|f| {
                let name = f.file_name().into_string().ok()?;
                name.strip_prefix("fan")?
                    .strip_suffix("_input")?
                    .parse()
                    .ok()
            })
            .collect();
        indices.sort_unstable();

        for n in indices {
            let Ok(raw) = std::fs::read_to_string(dir.join(format!("fan{n}_input"))) else {
                continue;
            };
            let Ok(rpm) = raw.trim().parse::<u32>() else {
                continue;
            };
            let label = std::fs::read_to_string(dir.join(format!("fan{n}_label")))
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .or_else(|| chip_name.clone())
                .unwrap_or_else(|| format!("Fan {n}"));
            fans.push(FanReading { label, rpm });
        }
    }
    fans.sort_by(|a, b| a.label.cmp(&b.label));
    fans
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
    fn canonical_encoding_is_pinned_and_agrees_with_core() {
        // The asus-wmi throttle_thermal_policy integer contract. The 90C thermal
        // guard (const PERFORMANCE = 1) and the platform_profile token mapping in
        // myasus-core all assume these exact values -- if any drift, this breaks.
        assert_eq!(FanProfile::Balanced.as_raw(), 0);
        assert_eq!(FanProfile::Performance.as_raw(), 1);
        assert_eq!(FanProfile::Quiet.as_raw(), 2);
        assert_eq!(
            myasus_core::profile_from_token("performance"),
            Some(FanProfile::Performance.as_raw())
        );
        assert_eq!(
            myasus_core::profile_from_token("balanced"),
            Some(FanProfile::Balanced.as_raw())
        );
        assert_eq!(
            myasus_core::profile_from_token("quiet"),
            Some(FanProfile::Quiet.as_raw())
        );
    }

    #[test]
    fn fan_profile_rejects_invalid() {
        assert!(FanProfile::from_raw(3).is_err());
        assert!(FanProfile::from_raw(255).is_err());
    }

    #[test]
    fn scan_fans_empty_without_sensors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_fans(dir.path()).is_empty());
    }

    // CPU-temp zone selection is now tested in myasus-core (cpu_temp/read_zones).

    #[test]
    fn scan_fans_reads_rpm_and_prefers_label() {
        let dir = tempfile::tempdir().unwrap();
        let hwmon = dir.path().join("hwmon3");
        std::fs::create_dir_all(&hwmon).unwrap();
        std::fs::write(hwmon.join("name"), "asus\n").unwrap();
        std::fs::write(hwmon.join("fan1_input"), "2400\n").unwrap();
        std::fs::write(hwmon.join("fan1_label"), "CPU Fan\n").unwrap();
        std::fs::write(hwmon.join("fan2_input"), "1800\n").unwrap();

        let fans = scan_fans(dir.path());
        assert_eq!(fans.len(), 2);
        // sorted alphabetically by label, so the labelled "CPU Fan" sorts ahead
        // of the chip-name fallback "asus"
        assert_eq!(
            fans[0],
            FanReading {
                label: "CPU Fan".to_owned(),
                rpm: 2400
            }
        );
        // fan2 has no label, falls back to the chip name
        assert_eq!(
            fans[1],
            FanReading {
                label: "asus".to_owned(),
                rpm: 1800
            }
        );
    }
}
