// Integration fixtures for battery resolution across real non-ROG sysfs layouts.
// Unit tests in the crate cover the cascade tiers; these assert the public API
// end-to-end over temp /sys trees shaped like actual machines.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

use myasus_core::{CHARGE_THRESHOLD_ATTR, battery_dir, charge_threshold_path};

/// Build a battery dir with a `type`, optional design energy, optional threshold.
fn battery(root: &Path, name: &str, energy_uwh: Option<u64>, threshold: Option<u8>) {
    let d = root.join(name);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("type"), "Battery\n").unwrap();
    if let Some(e) = energy_uwh {
        std::fs::write(d.join("energy_full_design"), format!("{e}\n")).unwrap();
    }
    if let Some(t) = threshold {
        std::fs::write(d.join(CHARGE_THRESHOLD_ATTR), format!("{t}\n")).unwrap();
    }
}

fn mains(root: &Path, name: &str) {
    let d = root.join(name);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("type"), "Mains\n").unwrap();
}

// Vivobook-style: the only battery is BAT1, and an AC adapter is present. The
// hardcoded-BAT0 bug made the whole battery dashboard vanish on these.
#[test]
fn vivobook_single_bat1() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mains(root, "ADP1");
    battery(root, "BAT1", Some(50_000_000), Some(80));
    assert_eq!(battery_dir(root), Some(root.join("BAT1")));
    assert_eq!(
        charge_threshold_path(root),
        Some(root.join("BAT1").join(CHARGE_THRESHOLD_ATTR))
    );
}

// Some firmwares name it BATT (no trailing index).
#[test]
fn batt_no_index() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    battery(root, "BATT", Some(48_000_000), Some(80));
    assert_eq!(battery_dir(root), Some(root.join("BATT")));
}

// Dual battery: the larger main pack must win over a small secondary cell, and
// among threshold-capable ones specifically.
#[test]
fn dual_battery_picks_main_pack() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    battery(root, "BAT0", Some(8_000_000), Some(80)); // small secondary
    battery(root, "BAT1", Some(70_000_000), Some(80)); // main
    assert_eq!(battery_dir(root), Some(root.join("BAT1")));
}

// Desktop: AC only, no battery at all.
#[test]
fn desktop_has_no_battery() {
    let dir = tempfile::tempdir().unwrap();
    mains(dir.path(), "AC0");
    assert_eq!(battery_dir(dir.path()), None);
    assert_eq!(charge_threshold_path(dir.path()), None);
}
