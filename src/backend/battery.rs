use super::detect;
use super::error::BackendError;
use super::sysfs;

pub const THRESHOLD_MIN: u8 = myasus_core::CHARGE_MIN;
pub const THRESHOLD_MAX: u8 = myasus_core::CHARGE_MAX;
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

/// Charging state, parsed once from the sysfs `status` string so call sites
/// match on a value instead of re-comparing free-form text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryStatus {
    fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("charging") {
            Self::Charging
        } else if raw.eq_ignore_ascii_case("discharging") {
            Self::Discharging
        } else if raw.eq_ignore_ascii_case("full") {
            Self::Full
        } else if raw.eq_ignore_ascii_case("not charging") {
            Self::NotCharging
        } else {
            Self::Unknown
        }
    }

    pub fn is_charging(self) -> bool {
        matches!(self, Self::Charging)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub capacity: u8,
    pub status: BatteryStatus,
    pub health_percent: f64,
    pub cycle_count: Option<u32>,
    pub charge_threshold: Option<u8>,
    pub voltage_mv: Option<u32>,
    pub current_ma: Option<i32>,
    /// Instantaneous power flow, watts (always positive).
    pub power_w: Option<f64>,
    /// Hours until full (charging) or empty (discharging), at the current rate.
    pub time_remaining_h: Option<f64>,
}

impl BatteryInfo {
    pub fn is_charging(&self) -> bool {
        self.status.is_charging()
    }
}

pub fn read_battery_info() -> Result<BatteryInfo, BackendError> {
    let capacity: u8 = sysfs::read_value(detect::BAT_CAPACITY)?;
    let status = BatteryStatus::parse(&sysfs::read(detect::BAT_STATUS)?);

    let charge_full = sysfs::read_value::<u64>(detect::BAT_CHARGE_FULL).ok();
    let health_percent = match (
        charge_full,
        sysfs::read_value::<f64>(detect::BAT_CHARGE_FULL_DESIGN),
    ) {
        (Some(full), Ok(design)) => health_from_charge(full, design),
        _ => 100.0,
    };

    let cycle_count = sysfs::read_value::<u32>(detect::BAT_CYCLE_COUNT).ok();
    let charge_threshold = sysfs::read_value::<u8>(detect::CHARGE_CONTROL_END_THRESHOLD).ok();

    // sysfs reports micro-volts / micro-amps / micro-amp-hours
    let voltage_uv = sysfs::read_value::<u64>(detect::BAT_VOLTAGE_NOW).ok();
    let current_ua = sysfs::read_value::<i64>(detect::BAT_CURRENT_NOW).ok();
    let charge_now = sysfs::read_value::<u64>(detect::BAT_CHARGE_NOW).ok();

    let power_w = match (voltage_uv, current_ua) {
        (Some(v), Some(i)) => Some(power_watts(v, i)),
        _ => None,
    };

    let charging = status.is_charging();
    let time_remaining_h = match (charge_now, charge_full, current_ua) {
        (Some(now), Some(full), Some(i)) => estimate_hours(now, full, i.unsigned_abs(), charging),
        _ => None,
    };

    Ok(BatteryInfo {
        capacity,
        status,
        health_percent,
        cycle_count,
        charge_threshold,
        voltage_mv: voltage_uv.map(|v| u32::try_from(v / 1000).unwrap_or(u32::MAX)),
        current_ma: current_ua.map(|i| i32::try_from(i / 1000).unwrap_or(i32::MAX)),
        power_w,
        time_remaining_h,
    })
}

/// Hours of runtime left, from charge counters and the present current draw.
fn estimate_hours(
    charge_now_uah: u64,
    charge_full_uah: u64,
    current_ua: u64,
    charging: bool,
) -> Option<f64> {
    if current_ua == 0 {
        return None;
    }
    let remaining = if charging {
        charge_full_uah.saturating_sub(charge_now_uah)
    } else {
        charge_now_uah
    };
    Some(remaining as f64 / current_ua as f64)
}

/// Battery health as a percentage of design capacity, clamped to 100. New cells
/// sometimes report `charge_full` above design, which would otherwise read as
/// over 100%.
fn health_from_charge(full_uah: u64, design_uah: f64) -> f64 {
    if design_uah > 0.0 {
        (full_uah as f64 / design_uah * 100.0).min(100.0)
    } else {
        100.0
    }
}

/// Instantaneous power draw in watts from micro-volts and micro-amps. The
/// current's sign (charge vs discharge) is dropped; this is magnitude only.
fn power_watts(voltage_uv: u64, current_ua: i64) -> f64 {
    (voltage_uv as f64 / 1e6) * (current_ua.unsigned_abs() as f64 / 1e6)
}

/// The charge limit currently programmed into the EC, if the control exists.
/// The daemon restores this on boot, so the live kernel value is the single
/// source of truth for the slider -- no separate config file to drift from it.
pub fn charge_threshold() -> Option<u8> {
    sysfs::read_value::<u8>(detect::CHARGE_CONTROL_END_THRESHOLD).ok()
}

pub fn set_charge_threshold(value: u8) -> Result<(), BackendError> {
    myasus_core::Op::ChargeThreshold(value).validate()?;
    super::daemon::set_charge_threshold(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_rejects_below_40() {
        let err = set_charge_threshold(39).unwrap_err();
        assert!(matches!(
            err,
            BackendError::Validate(myasus_core::ValidateError::ChargeThreshold(39))
        ));
    }

    #[test]
    fn time_to_empty_uses_charge_now() {
        // 2000 mAh left, drawing 1000 mA -> 2 hours
        let h = estimate_hours(2_000_000, 4_000_000, 1_000_000, false).unwrap();
        assert!((h - 2.0).abs() < 1e-9);
    }

    #[test]
    fn time_to_full_uses_headroom() {
        // 1000 mAh to go to 4000 mAh full, at 2000 mA -> 0.5 hours
        let h = estimate_hours(3_000_000, 4_000_000, 2_000_000, true).unwrap();
        assert!((h - 0.5).abs() < 1e-9);
    }

    #[test]
    fn no_estimate_at_zero_current() {
        assert!(estimate_hours(2_000_000, 4_000_000, 0, false).is_none());
    }

    #[test]
    fn threshold_rejects_above_100() {
        let err = set_charge_threshold(101).unwrap_err();
        assert!(matches!(
            err,
            BackendError::Validate(myasus_core::ValidateError::ChargeThreshold(101))
        ));
    }

    #[test]
    fn health_status_bands_and_boundaries() {
        assert_eq!(HealthStatus::from_percent(95.0), HealthStatus::Good);
        assert_eq!(HealthStatus::from_percent(80.0), HealthStatus::Good); // exact Good edge
        assert_eq!(HealthStatus::from_percent(79.9), HealthStatus::Fair);
        assert_eq!(HealthStatus::from_percent(50.0), HealthStatus::Fair); // exact Fair edge
        assert_eq!(HealthStatus::from_percent(49.9), HealthStatus::ReplaceSoon);
        assert_eq!(HealthStatus::from_percent(30.0), HealthStatus::ReplaceSoon);
    }

    #[test]
    fn health_is_ratio_of_design_clamped_to_100() {
        assert!((health_from_charge(3_000_000, 4_000_000.0) - 75.0).abs() < 1e-9);
        // a cell reporting above design must not read over 100%
        assert!((health_from_charge(5_000_000, 4_000_000.0) - 100.0).abs() < 1e-9);
        // a missing/zero design falls back rather than dividing by zero
        assert!((health_from_charge(3_000_000, 0.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn power_watts_uses_current_magnitude() {
        // 12 V * 2 A = 24 W, whether charging (+) or discharging (-)
        assert!((power_watts(12_000_000, 2_000_000) - 24.0).abs() < 1e-9);
        assert!((power_watts(12_000_000, -2_000_000) - 24.0).abs() < 1e-9);
    }

    #[test]
    fn status_parses_known_strings_and_charging() {
        assert_eq!(BatteryStatus::parse("Charging\n"), BatteryStatus::Charging);
        assert_eq!(
            BatteryStatus::parse("DISCHARGING"),
            BatteryStatus::Discharging
        );
        assert_eq!(
            BatteryStatus::parse("Not charging"),
            BatteryStatus::NotCharging
        );
        assert_eq!(BatteryStatus::parse("weird"), BatteryStatus::Unknown);
        assert!(BatteryStatus::parse("charging").is_charging());
        assert!(!BatteryStatus::parse("Full").is_charging());
    }
}
