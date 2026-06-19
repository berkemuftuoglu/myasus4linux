//! The non-negotiable safety policies from CLAUDE.md, expressed as pure
//! decisions so the UI only has to enact them and the rules stay unit-testable.

use super::fan::FanProfile;

/// Any thermal zone at or above this temperature (Celsius) forces maximum
/// cooling regardless of the user's chosen profile.
pub const THERMAL_LIMIT_C: f64 = 90.0;

/// At or below this battery percentage, on battery power, we suggest the quiet
/// (lower-power) profile.
pub const LOW_BATTERY_PCT: u8 = 20;

/// The profile to force when the machine is too hot to leave cooling to the
/// user, or `None` when no override is needed. Forcing `Performance` spins the
/// fans up; it is skipped when already there so the safeguard can't thrash.
pub fn thermal_override(max_temp_c: f64, current: FanProfile) -> Option<FanProfile> {
    if max_temp_c >= THERMAL_LIMIT_C && current != FanProfile::Performance {
        Some(FanProfile::Performance)
    } else {
        None
    }
}

/// Whether to suggest quiet mode: only on battery, only when low, and only if
/// not already quiet, so we never nag about a choice the user already made.
pub fn suggest_quiet(capacity: u8, charging: bool, current: FanProfile) -> bool {
    capacity <= LOW_BATTERY_PCT && !charging && current != FanProfile::Quiet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thermal_override_forces_cooling_only_when_hot_and_not_already() {
        assert_eq!(
            thermal_override(91.0, FanProfile::Quiet),
            Some(FanProfile::Performance)
        );
        assert_eq!(
            thermal_override(90.0, FanProfile::Balanced),
            Some(FanProfile::Performance)
        );
        assert_eq!(thermal_override(89.9, FanProfile::Quiet), None);
        // already at max cooling -> nothing to do, so no write thrash
        assert_eq!(thermal_override(95.0, FanProfile::Performance), None);
    }

    #[test]
    fn suggest_quiet_only_when_low_and_discharging_and_not_already_quiet() {
        assert!(suggest_quiet(20, false, FanProfile::Balanced));
        assert!(suggest_quiet(5, false, FanProfile::Performance));
        assert!(!suggest_quiet(20, true, FanProfile::Balanced)); // charging
        assert!(!suggest_quiet(21, false, FanProfile::Balanced)); // not low
        assert!(!suggest_quiet(10, false, FanProfile::Quiet)); // already quiet
    }
}
