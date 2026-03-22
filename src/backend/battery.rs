use super::detect;
use super::error::BackendError;
use super::sysfs;

/// Snapshot of current battery information.
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// Current charge percentage (0-100).
    pub capacity: u8,
    /// Charging status string (e.g. "Charging", "Discharging", "Full").
    pub status: String,
    /// Battery health as a percentage of design capacity.
    pub health_percent: f64,
    /// Number of charge cycles completed.
    pub cycle_count: Option<u32>,
    /// Current charge limit threshold (if supported).
    pub charge_threshold: Option<u8>,
}

/// Read all available battery information from sysfs.
pub fn read_battery_info() -> Result<BatteryInfo, BackendError> {
    let capacity: u8 = sysfs::read_value(detect::BAT_CAPACITY)?;
    let status = sysfs::read(detect::BAT_STATUS)?;

    let health_percent = match (
        sysfs::read_value::<f64>(detect::BAT_CHARGE_FULL),
        sysfs::read_value::<f64>(detect::BAT_CHARGE_FULL_DESIGN),
    ) {
        (Ok(full), Ok(design)) if design > 0.0 => (full / design) * 100.0,
        _ => 100.0,
    };

    let cycle_count = sysfs::read_value::<u32>(detect::BAT_CYCLE_COUNT).ok();

    let charge_threshold =
        sysfs::read_value::<u8>(detect::CHARGE_CONTROL_END_THRESHOLD).ok();

    Ok(BatteryInfo {
        capacity,
        status,
        health_percent,
        cycle_count,
        charge_threshold,
    })
}

/// Set the battery charge end threshold.
///
/// # Safeguards
/// - Values below 40 are rejected (`InvalidThreshold`).
/// - Values above 100 are rejected (`InvalidThreshold`).
/// - Uses pkexec for privilege escalation.
pub fn set_charge_threshold(value: u8) -> Result<(), BackendError> {
    if !(40..=100).contains(&value) {
        return Err(BackendError::InvalidThreshold(value));
    }

    sysfs::write_privileged(
        detect::CHARGE_CONTROL_END_THRESHOLD,
        &value.to_string(),
    )
}

/// Return a plain-English health label based on health percentage.
pub fn health_label(health_percent: f64) -> &'static str {
    if health_percent >= 80.0 {
        "Good"
    } else if health_percent >= 50.0 {
        "Fair"
    } else {
        "Replace soon"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_rejects_below_40() {
        let err = set_charge_threshold(39).unwrap_err();
        assert!(matches!(err, BackendError::InvalidThreshold(39)));
    }

    #[test]
    fn threshold_rejects_above_100() {
        let err = set_charge_threshold(101).unwrap_err();
        assert!(matches!(err, BackendError::InvalidThreshold(101)));
    }

    #[test]
    fn health_labels() {
        assert_eq!(health_label(95.0), "Good");
        assert_eq!(health_label(65.0), "Fair");
        assert_eq!(health_label(30.0), "Replace soon");
    }
}
