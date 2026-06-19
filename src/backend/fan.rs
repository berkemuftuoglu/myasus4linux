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
}

pub fn read_profile() -> Result<FanProfile, BackendError> {
    let raw: u8 = sysfs::read_value(detect::THROTTLE_THERMAL_POLICY)?;
    FanProfile::from_raw(raw)
}

/// Find the CPU thermal zone and return its temp in degrees C.
/// Scans `/sys/class/thermal/thermal_zone*/type` for known CPU zone names.
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
    // Try a direct write first (a udev rule may make the control user-writable);
    // fall back to pkexec only when it is still root-owned.
    let value = (profile as u8).to_string();
    sysfs::write(detect::THROTTLE_THERMAL_POLICY, &value)
        .or_else(|_| sysfs::write_privileged(detect::THROTTLE_THERMAL_POLICY, &value))
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
    fn fan_profile_rejects_invalid() {
        assert!(FanProfile::from_raw(3).is_err());
        assert!(FanProfile::from_raw(255).is_err());
    }

    #[test]
    fn scan_fans_empty_without_sensors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_fans(dir.path()).is_empty());
    }

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
        // sorted by label: "CPU Fan" (labelled) then "asus" -> actually alpha: "CPU Fan" < "asus"
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
