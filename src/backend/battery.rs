use super::detect;
use super::error::BackendError;
use super::sysfs;

pub const THRESHOLD_MIN: u8 = 40;
pub const THRESHOLD_MAX: u8 = 100;
pub const THRESHOLD_DEFAULT: u8 = 80;

const HEALTH_GOOD: f64 = 80.0;
const HEALTH_FAIR: f64 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Good,
    Fair,
    ReplaceSoon,
}

impl HealthStatus {
    pub fn from_percent(health: f64) -> Self {
        if health >= HEALTH_GOOD {
            Self::Good
        } else if health >= HEALTH_FAIR {
            Self::Fair
        } else {
            Self::ReplaceSoon
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "Good",
            Self::Fair => "Fair",
            Self::ReplaceSoon => "Replace soon",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub capacity: u8,
    /// E.g. "Charging", "Discharging", "Full".
    pub status: String,
    pub health_percent: f64,
    pub cycle_count: Option<u32>,
    pub charge_threshold: Option<u8>,
    pub voltage_mv: Option<u32>,
    pub current_ma: Option<i32>,
}

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

    // sysfs reports microvolts/microamps
    let voltage_mv = sysfs::read_value::<u32>(detect::BAT_VOLTAGE_NOW)
        .ok()
        .map(|v| v / 1000);
    let current_ma = sysfs::read_value::<i32>(detect::BAT_CURRENT_NOW)
        .ok()
        .map(|v| v / 1000);

    Ok(BatteryInfo {
        capacity,
        status,
        health_percent,
        cycle_count,
        charge_threshold,
        voltage_mv,
        current_ma,
    })
}

pub fn set_charge_threshold(value: u8) -> Result<(), BackendError> {
    if !(THRESHOLD_MIN..=THRESHOLD_MAX).contains(&value) {
        return Err(BackendError::InvalidThreshold(value));
    }
    sysfs::write_privileged(
        detect::CHARGE_CONTROL_END_THRESHOLD,
        &value.to_string(),
    )
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
    fn health_status() {
        assert_eq!(HealthStatus::from_percent(95.0), HealthStatus::Good);
        assert_eq!(HealthStatus::from_percent(65.0), HealthStatus::Fair);
        assert_eq!(HealthStatus::from_percent(30.0), HealthStatus::ReplaceSoon);
    }
}
