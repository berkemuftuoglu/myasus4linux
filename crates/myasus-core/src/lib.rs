// Panics/indexing are fine in tests; the panic-class lints only guard production.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

//! Shared contract for the privileged operations the root helper performs.
//!
//! Paths, value ranges, and serialisation live here in exactly one place. The
//! daemon validates through this and writes the fixed path; a new privileged
//! feature adds a variant here rather than inventing another ad-hoc escalation
//! path. Nothing here trusts a caller-supplied path. This crate is GTK-free;
//! the only I/O is the battery-directory resolver, which takes an injectable
//! root so it stays temp-dir testable.

use std::path::{Path, PathBuf};

/// Root under which the kernel exposes batteries and AC adapters. The battery is
/// NOT always `BAT0` (real non-ROG laptops use `BAT1`/`BATT`/`BATC`), so the
/// device is resolved at runtime by [`battery_dir`] rather than hardcoded.
pub const POWER_SUPPLY_ROOT: &str = "/sys/class/power_supply";
/// The charge-limit control, relative to the resolved battery directory.
pub const CHARGE_THRESHOLD_ATTR: &str = "charge_control_end_threshold";
pub const FAN_PROFILE_PATH: &str = "/sys/devices/platform/asus-nb-wmi/throttle_thermal_policy";
/// Kernel-standard ACPI performance interface, the fallback when the ASUS WMI
/// `throttle_thermal_policy` is absent (more non-ROG laptops expose this one).
pub const PLATFORM_PROFILE_PATH: &str = "/sys/firmware/acpi/platform_profile";
/// Space-separated tokens this machine's `platform_profile` accepts.
pub const PLATFORM_PROFILE_CHOICES_PATH: &str = "/sys/firmware/acpi/platform_profile_choices";
pub const KBD_BACKLIGHT_PATH: &str = "/sys/class/leds/asus::kbd_backlight/brightness";
/// Root under which the kernel exposes LEDs. The keyboard backlight's sysname is
/// vendor-prefixed and varies, so it is matched at runtime by [`kbd_backlight_path`].
pub const LEDS_ROOT: &str = "/sys/class/leds";

/// The well-known D-Bus name the privileged helper owns on the system bus, and
/// the object path it serves. The daemon connects/serves with these; the client
/// proxy targets them. NOTE: `#[zbus::interface]`/`#[zbus::proxy]` attribute
/// strings cannot reference a const, so the literals there must stay identical
/// to these.
pub const DBUS_NAME: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper";
pub const DBUS_PATH: &str = "/io/github/berkmuftuoglu/MyAsus4Linux/Helper";

/// Charge limit bounds. Below `CHARGE_MIN` the battery can over-discharge; the
/// kernel rejects anything outside this, but owning the range here means the
/// daemon and the GUI agree on one definition instead of three.
pub const CHARGE_MIN: u8 = 40;
pub const CHARGE_MAX: u8 = 100;
/// Safeguard #1: the charge limit applied on first run (no persisted value) when
/// the control exists, so battery longevity is protected out of the box.
pub const CHARGE_DEFAULT: u8 = 80;

/// One privileged write. Carries its own value; the target path is fixed per
/// variant and never comes from the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    ChargeThreshold(u8),
    FanProfile(u8),
    KeyboardBacklight(u8),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidateError {
    #[error("charge threshold {0} out of range (40-100)")]
    ChargeThreshold(u8),
    #[error("fan profile {0} out of range (0-2)")]
    FanProfile(u8),
    #[error("keyboard brightness {0} out of range (0-3)")]
    KeyboardBacklight(u8),
}

impl Op {
    /// Range-check the value. The kernel also checks, but validating here means
    /// the daemon never even attempts an out-of-range write.
    pub fn validate(self) -> Result<(), ValidateError> {
        match self {
            Op::ChargeThreshold(v) if !(40..=100).contains(&v) => {
                Err(ValidateError::ChargeThreshold(v))
            }
            Op::FanProfile(v) if v > 2 => Err(ValidateError::FanProfile(v)),
            Op::KeyboardBacklight(v) if v > 3 => Err(ValidateError::KeyboardBacklight(v)),
            _ => Ok(()),
        }
    }
}

/// Resolve the main battery's sysfs directory under `root` (normally
/// [`POWER_SUPPLY_ROOT`]). Follows asusctl's cascade: prefer the device exposing
/// the charge-threshold control, else a sysname starting `BAT`, else any
/// `type == Battery`. When several qualify, the largest design capacity wins so
/// a tiny secondary cell or UPS can't shadow the main pack. `None` on a desktop
/// with no battery.
pub fn battery_dir(root: &Path) -> Option<PathBuf> {
    let batteries: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_battery(p))
        .collect();

    let with_attr: Vec<PathBuf> = batteries
        .iter()
        .filter(|p| p.join(CHARGE_THRESHOLD_ATTR).exists())
        .cloned()
        .collect();
    if let Some(best) = largest_capacity(&with_attr) {
        return Some(best);
    }

    let named: Vec<PathBuf> = batteries
        .iter()
        .filter(|p| sysname_starts_with_bat(p))
        .cloned()
        .collect();
    if let Some(best) = largest_capacity(&named) {
        return Some(best);
    }

    largest_capacity(&batteries)
}

/// Full path to the charge-limit control on the resolved battery, if present.
pub fn charge_threshold_path(root: &Path) -> Option<PathBuf> {
    Some(battery_dir(root)?.join(CHARGE_THRESHOLD_ATTR))
}

/// Resolve the keyboard backlight `brightness` attribute under `root` (normally
/// [`LEDS_ROOT`]). Matches any LED whose sysname contains `kbd_backlight`, since
/// the vendor prefix varies (`asus::kbd_backlight`, ...). `None` if absent.
pub fn kbd_backlight_path(root: &Path) -> Option<PathBuf> {
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("kbd_backlight"))
        })
        .map(|p| p.join("brightness"))
}

/// Whether the machine is on external power, by scanning every non-battery
/// supply's `online` flag under `root`. This covers a classic `Mains` adapter
/// (AC/ADP/ACAD) AND USB-C PD charging, where the source reports `type == USB`
/// -- many thin ASUS laptops have no `Mains` device at all. `None` when nothing
/// exposes an `online` attribute, so the caller cannot tell.
pub fn on_external_power(root: &Path) -> Option<bool> {
    let mut saw_online = false;
    let mut online = false;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let p = entry.path();
        if is_battery(&p) {
            continue;
        }
        if let Ok(v) = std::fs::read_to_string(p.join("online")) {
            saw_online = true;
            online |= v.trim() == "1";
        }
    }
    saw_online.then_some(online)
}

/// The daemon's persisted settings, re-applied on boot and after resume. A
/// plain `key=value` text format (not a real config language) keeps this crate
/// dependency-free; unknown keys are ignored for forward compatibility, and a
/// bare integer is read as the legacy charge-only file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DaemonState {
    pub charge_threshold: Option<u8>,
    pub fan_profile: Option<u8>,
    pub kbd_backlight: Option<u8>,
}

impl DaemonState {
    pub fn parse(raw: &str) -> Self {
        // Legacy charge-only file: a bare number with no key.
        if let Ok(v) = raw.trim().parse::<u8>() {
            return Self {
                charge_threshold: Some(v),
                ..Self::default()
            };
        }
        let mut state = Self::default();
        for line in raw.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().parse().ok();
            match key.trim() {
                "charge_threshold" => state.charge_threshold = value,
                "fan_profile" => state.fan_profile = value,
                "kbd_backlight" => state.kbd_backlight = value,
                _ => {}
            }
        }
        state
    }

    pub fn serialize(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for (key, value) in [
            ("charge_threshold", self.charge_threshold),
            ("fan_profile", self.fan_profile),
            ("kbd_backlight", self.kbd_backlight),
        ] {
            if let Some(v) = value {
                let _ = writeln!(out, "{key}={v}");
            }
        }
        out
    }

    /// Record an op's value into the matching field.
    pub fn set(&mut self, op: Op) {
        match op {
            Op::ChargeThreshold(v) => self.charge_threshold = Some(v),
            Op::FanProfile(v) => self.fan_profile = Some(v),
            Op::KeyboardBacklight(v) => self.kbd_backlight = Some(v),
        }
    }

    /// The persisted settings as ops, charge first so the limit is restored
    /// before anything else.
    pub fn ops(&self) -> Vec<Op> {
        let mut ops = Vec::new();
        if let Some(v) = self.charge_threshold {
            ops.push(Op::ChargeThreshold(v));
        }
        if let Some(v) = self.fan_profile {
            ops.push(Op::FanProfile(v));
        }
        if let Some(v) = self.kbd_backlight {
            ops.push(Op::KeyboardBacklight(v));
        }
        ops
    }
}

/// Root under which the kernel exposes thermal zones.
pub const THERMAL_ROOT: &str = "/sys/class/thermal";

/// A thermal sensor reading: the raw zone `kind` (its `type` string) and Celsius.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub kind: String,
    pub celsius: f64,
}

/// Read all thermal zones under `root` (normally [`THERMAL_ROOT`]). Implausibly
/// high readings -- sensor-unavailable sentinels (~128C) or garbage -- are
/// dropped so they can't trip the thermal guard or show a bogus temperature.
/// One parse type (f64) shared by every caller.
pub fn read_zones(root: &Path) -> Vec<Zone> {
    const MAX_PLAUSIBLE_C: f64 = 115.0;
    let mut zones = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return zones;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("thermal_zone"))
        {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(dir.join("temp")) else {
            continue;
        };
        let Ok(milli) = raw.trim().parse::<f64>() else {
            continue;
        };
        let celsius = milli / 1000.0;
        if celsius > MAX_PLAUSIBLE_C {
            continue;
        }
        let kind = std::fs::read_to_string(dir.join("type"))
            .map_or_else(|_| "Sensor".to_owned(), |s| s.trim().to_owned());
        zones.push(Zone { kind, celsius });
    }
    zones
}

/// The hottest plausible zone, if any. The basis for the thermal guard/safeguard.
pub fn hottest_zone(root: &Path) -> Option<Zone> {
    read_zones(root)
        .into_iter()
        .max_by(|a, b| a.celsius.total_cmp(&b.celsius))
}

/// The CPU temperature, preferring package/core sensors over the generic ACPI
/// zone when several exist.
pub fn cpu_temp(root: &Path) -> Option<f64> {
    const PRIORITY: [&str; 4] = ["x86_pkg_temp", "coretemp", "TCPU", "acpitz"];
    read_zones(root)
        .into_iter()
        .filter_map(|z| {
            PRIORITY
                .iter()
                .position(|p| *p == z.kind)
                .map(|rank| (rank, z.celsius))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, c)| c)
}

/// Any thermal zone at or above this (Celsius) forces maximum cooling regardless
/// of the user's chosen profile. Shared by the GUI safeguard and the daemon's
/// headless guard so both agree on the policy.
pub const THERMAL_LIMIT_C: f64 = 90.0;

/// The profile to force when too hot, or `None` when no override is needed.
/// `current` and the return are canonical fan-profile values (1 == performance
/// == max cooling). Skipped when already at performance so it cannot thrash.
pub fn thermal_override(max_temp_c: f64, current: u8) -> Option<u8> {
    const PERFORMANCE: u8 = 1;
    (max_temp_c >= THERMAL_LIMIT_C && current != PERFORMANCE).then_some(PERFORMANCE)
}

/// What the daemon's thermal guard should do this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalAction {
    None,
    /// Force maximum cooling (performance).
    Force,
    /// Cooled down: restore this canonical profile (the snapshot taken when the
    /// override began; the daemon may prefer the persisted user intent).
    Restore(u8),
}

/// Stateful thermal-guard decision, kept pure so the hysteresis is unit-tested.
/// `overridden` is the profile we forced away from (None when not overriding).
/// Returns the action and the new override state. Force at/above the limit,
/// restore only once it cools a margin below (no flapping), hold in between, and
/// relinquish without touching anything if the profile changed out from under us
/// (the user took control mid-episode).
pub fn guard_step(max_c: f64, current: u8, overridden: Option<u8>) -> (ThermalAction, Option<u8>) {
    const PERFORMANCE: u8 = 1;
    const RESTORE_BELOW_C: f64 = THERMAL_LIMIT_C - 5.0;
    match overridden {
        None if max_c >= THERMAL_LIMIT_C && current != PERFORMANCE => {
            (ThermalAction::Force, Some(current))
        }
        None => (ThermalAction::None, None),
        Some(_) if current != PERFORMANCE => (ThermalAction::None, None),
        Some(snapshot) if max_c < RESTORE_BELOW_C => (ThermalAction::Restore(snapshot), None),
        Some(snapshot) => (ThermalAction::None, Some(snapshot)),
    }
}

/// Canonical fan-profile value for a `platform_profile` token (0=balanced,
/// 1=performance, 2=quiet, where `low-power` collapses into quiet), or `None`
/// for an unknown token. The single mapping the GUI and daemon both use.
pub fn profile_from_token(token: &str) -> Option<u8> {
    match token.trim() {
        "balanced" => Some(0),
        "performance" => Some(1),
        "quiet" | "low-power" => Some(2),
        _ => None,
    }
}

/// The `platform_profile` tokens that satisfy a canonical value, best first.
/// Quiet resolves to whichever of `quiet`/`low-power` the firmware exposes.
pub fn profile_tokens(value: u8) -> &'static [&'static str] {
    match value {
        0 => &["balanced"],
        1 => &["performance"],
        2 => &["quiet", "low-power"],
        _ => &[],
    }
}

fn is_battery(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("type"))
        .is_ok_and(|t| t.trim().eq_ignore_ascii_case("Battery"))
}

fn sysname_starts_with_bat(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_uppercase().starts_with("BAT"))
}

/// Design capacity as a comparable magnitude (energy uWh if present, else charge
/// uAh) used only to rank candidate batteries, never as an absolute value.
fn design_capacity(dir: &Path) -> u64 {
    read_u64(dir, "energy_full_design")
        .or_else(|| read_u64(dir, "charge_full_design"))
        .unwrap_or(0)
}

fn read_u64(dir: &Path, attr: &str) -> Option<u64> {
    std::fs::read_to_string(dir.join(attr))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn largest_capacity(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .max_by_key(|p| design_capacity(p))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_threshold_range() {
        assert!(Op::ChargeThreshold(40).validate().is_ok());
        assert!(Op::ChargeThreshold(80).validate().is_ok());
        assert!(Op::ChargeThreshold(100).validate().is_ok());
        assert_eq!(
            Op::ChargeThreshold(39).validate(),
            Err(ValidateError::ChargeThreshold(39))
        );
        assert_eq!(
            Op::ChargeThreshold(101).validate(),
            Err(ValidateError::ChargeThreshold(101))
        );
    }

    #[test]
    fn fan_profile_range() {
        for v in 0..=2 {
            assert!(Op::FanProfile(v).validate().is_ok());
        }
        assert_eq!(
            Op::FanProfile(3).validate(),
            Err(ValidateError::FanProfile(3))
        );
    }

    #[test]
    fn keyboard_range() {
        for v in 0..=3 {
            assert!(Op::KeyboardBacklight(v).validate().is_ok());
        }
        assert_eq!(
            Op::KeyboardBacklight(4).validate(),
            Err(ValidateError::KeyboardBacklight(4))
        );
    }

    #[test]
    fn fixed_path_consts_are_absolute_sys_attributes() {
        // Every write target the daemon can resolve sits under /sys; the unit's
        // ReadWritePaths must cover each of these roots (see myasusd.service.in).
        for path in [FAN_PROFILE_PATH, PLATFORM_PROFILE_PATH, KBD_BACKLIGHT_PATH] {
            assert!(path.starts_with("/sys/"), "{path} escapes /sys");
            assert!(Path::new(path).is_absolute());
        }
    }

    #[test]
    fn battery_dir_prefers_threshold_then_bat_then_largest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // BAT0: a battery but no charge-threshold control.
        mk_battery(root, "BAT0", Some(50_000_000), None);
        // BAT1: exposes the threshold control -> wins tier 1.
        mk_battery(root, "BAT1", Some(60_000_000), Some(80));
        assert_eq!(battery_dir(root), Some(root.join("BAT1")));
    }

    #[test]
    fn battery_dir_skips_tiny_secondary_cell() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A tiny UPS-like cell with the control, and the real main pack with it.
        mk_battery(root, "BATT", Some(5_000_000), Some(80));
        mk_battery(root, "BAT0", Some(62_000_000), Some(80));
        assert_eq!(battery_dir(root), Some(root.join("BAT0")));
    }

    #[test]
    fn battery_dir_none_without_a_battery() {
        let dir = tempfile::tempdir().unwrap();
        // An AC adapter is present but no battery (desktop).
        let ac = dir.path().join("AC0");
        std::fs::create_dir_all(&ac).unwrap();
        std::fs::write(ac.join("type"), "Mains\n").unwrap();
        assert_eq!(battery_dir(dir.path()), None);
    }

    #[test]
    fn charge_threshold_path_joins_resolved_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        mk_battery(root, "BATC", Some(50_000_000), Some(80));
        assert_eq!(
            charge_threshold_path(root),
            Some(root.join("BATC").join(CHARGE_THRESHOLD_ATTR))
        );
    }

    #[test]
    fn external_power_detects_usb_c_not_just_mains() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        mk_battery(root, "BAT0", Some(50_000_000), Some(80));
        // A USB-C PD source plugged in (type=USB, online=1), no Mains device --
        // exactly the shape of a thin ASUS laptop on its charger.
        let usbc = root.join("ucsi-source-psy");
        std::fs::create_dir_all(&usbc).unwrap();
        std::fs::write(usbc.join("type"), "USB\n").unwrap();
        std::fs::write(usbc.join("online"), "1\n").unwrap();
        assert_eq!(on_external_power(root), Some(true));
    }

    #[test]
    fn external_power_none_without_online_attrs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        mk_battery(root, "BAT0", Some(50_000_000), Some(80));
        assert_eq!(on_external_power(root), None);
    }

    #[test]
    fn kbd_backlight_matches_vendor_prefixed_led() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("input3::capslock")).unwrap();
        let kbd = root.join("asus::kbd_backlight");
        std::fs::create_dir_all(&kbd).unwrap();
        assert_eq!(kbd_backlight_path(root), Some(kbd.join("brightness")));
    }

    #[test]
    fn kbd_backlight_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("input3::scrolllock")).unwrap();
        assert_eq!(kbd_backlight_path(dir.path()), None);
    }

    #[test]
    fn profile_token_mapping_round_trips() {
        assert_eq!(profile_from_token("balanced"), Some(0));
        assert_eq!(profile_from_token("performance"), Some(1));
        assert_eq!(profile_from_token("quiet"), Some(2));
        assert_eq!(profile_from_token("low-power"), Some(2)); // collapses into quiet
        assert_eq!(profile_from_token(" performance\n"), Some(1)); // trims
        assert_eq!(profile_from_token("turbo"), None);
        assert_eq!(profile_tokens(2), &["quiet", "low-power"]);
        assert!(profile_tokens(9).is_empty());
    }

    #[test]
    fn thermal_override_forces_performance_only_when_hot() {
        assert_eq!(thermal_override(91.0, 0), Some(1));
        assert_eq!(thermal_override(90.0, 2), Some(1));
        assert_eq!(thermal_override(89.9, 0), None);
        assert_eq!(thermal_override(95.0, 1), None); // already at performance
    }

    fn mk_zone(root: &Path, name: &str, kind: &str, milli: &str) {
        let z = root.join(name);
        std::fs::create_dir_all(&z).unwrap();
        std::fs::write(z.join("type"), format!("{kind}\n")).unwrap();
        std::fs::write(z.join("temp"), format!("{milli}\n")).unwrap();
    }

    #[test]
    fn thermal_clamps_sentinel_and_prioritises_cpu_zone() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        mk_zone(root, "thermal_zone0", "acpitz", "55000");
        mk_zone(root, "thermal_zone1", "x86_pkg_temp", "61000");
        mk_zone(root, "thermal_zone2", "iwlwifi", "128000"); // sensor-unavailable sentinel
        // The 128C sentinel must be dropped so it can't trip the guard.
        assert_eq!(hottest_zone(root).map(|z| z.celsius), Some(61.0));
        // CPU temp prefers x86_pkg_temp over acpitz regardless of order.
        assert_eq!(cpu_temp(root), Some(61.0));
        assert_eq!(read_zones(root).len(), 2);
    }

    #[test]
    fn guard_step_hysteresis_and_relinquish() {
        use ThermalAction::{Force, None as NoAct, Restore};
        // not overriding yet
        assert_eq!(guard_step(89.9, 0, None), (NoAct, None)); // below limit
        assert_eq!(guard_step(90.0, 0, None), (Force, Some(0))); // force, remember balanced
        assert_eq!(guard_step(95.0, 1, None), (NoAct, None)); // already performance
        // overriding (snapshot = balanced/0)
        assert_eq!(guard_step(86.0, 1, Some(0)), (NoAct, Some(0))); // in hysteresis band -> hold
        assert_eq!(guard_step(84.9, 1, Some(0)), (Restore(0), None)); // cooled -> restore
        assert_eq!(guard_step(84.0, 2, Some(0)), (NoAct, None)); // user switched to quiet -> relinquish
    }

    #[test]
    fn state_round_trips_all_fields() {
        let s = DaemonState {
            charge_threshold: Some(80),
            fan_profile: Some(1),
            kbd_backlight: Some(2),
        };
        assert_eq!(DaemonState::parse(&s.serialize()), s);
    }

    #[test]
    fn state_reads_legacy_bare_charge_file() {
        assert_eq!(
            DaemonState::parse("80\n"),
            DaemonState {
                charge_threshold: Some(80),
                ..DaemonState::default()
            }
        );
    }

    #[test]
    fn state_ignores_unknown_keys_and_unparseable_values() {
        let s = DaemonState::parse("charge_threshold=70\nfuture_key=x\nfan_profile=oops\n");
        assert_eq!(s.charge_threshold, Some(70));
        assert_eq!(s.fan_profile, None);
    }

    #[test]
    fn state_ops_charge_first() {
        let s = DaemonState {
            charge_threshold: Some(80),
            fan_profile: Some(1),
            kbd_backlight: Some(2),
        };
        assert_eq!(
            s.ops(),
            vec![
                Op::ChargeThreshold(80),
                Op::FanProfile(1),
                Op::KeyboardBacklight(2)
            ]
        );
    }

    fn mk_battery(root: &Path, name: &str, energy_full_design: Option<u64>, threshold: Option<u8>) {
        let d = root.join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("type"), "Battery\n").unwrap();
        if let Some(e) = energy_full_design {
            std::fs::write(d.join("energy_full_design"), format!("{e}\n")).unwrap();
        }
        if let Some(t) = threshold {
            std::fs::write(d.join(CHARGE_THRESHOLD_ATTR), format!("{t}\n")).unwrap();
        }
    }
}
