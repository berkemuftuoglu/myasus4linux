//! Every thermal sensor the kernel exposes, not just the CPU package. The raw
//! enumeration (and implausible-reading clamp) lives in `myasus-core`; this layer
//! prettifies zone names and sorts hottest-first for the dashboard.

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalZone {
    pub label: String,
    pub celsius: f64,
}

pub fn read_zones() -> Vec<ThermalZone> {
    scan(Path::new(myasus_core::THERMAL_ROOT))
}

fn scan(root: &Path) -> Vec<ThermalZone> {
    let mut zones: Vec<ThermalZone> = myasus_core::read_zones(root)
        .into_iter()
        .map(|z| ThermalZone {
            label: pretty(&z.kind),
            celsius: z.celsius,
        })
        .collect();
    zones.sort_by(|a, b| b.celsius.total_cmp(&a.celsius));
    zones
}

/// Turn raw zone types into something readable (`x86_pkg_temp` -> `CPU Package`).
fn pretty(zone_type: &str) -> String {
    match zone_type {
        "x86_pkg_temp" => "CPU Package".to_owned(),
        "TCPU" => "CPU".to_owned(),
        "acpitz" => "Mainboard".to_owned(),
        other if other.starts_with("iwlwifi") => "Wi-Fi".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_empty_without_zones() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan(dir.path()).is_empty());
    }

    #[test]
    fn scan_reads_and_sorts_hottest_first() {
        let dir = tempfile::tempdir().unwrap();
        for (i, (ty, temp)) in [("x86_pkg_temp", "61000"), ("acpitz", "84000")]
            .iter()
            .enumerate()
        {
            let zone = dir.path().join(format!("thermal_zone{i}"));
            std::fs::create_dir_all(&zone).unwrap();
            std::fs::write(zone.join("type"), ty).unwrap();
            std::fs::write(zone.join("temp"), temp).unwrap();
        }
        let zones = scan(dir.path());
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[0].label, "Mainboard"); // 84C, hottest first
        assert!((zones[0].celsius - 84.0).abs() < 1e-9);
        assert_eq!(zones[1].label, "CPU Package");
    }
}
