//! Every thermal sensor the kernel exposes, not just the CPU package.

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalZone {
    pub label: String,
    pub celsius: f64,
}

pub fn read_zones() -> Vec<ThermalZone> {
    scan(Path::new("/sys/class/thermal"))
}

fn scan(root: &Path) -> Vec<ThermalZone> {
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
        let Ok(millideg) = raw.trim().parse::<i64>() else {
            continue;
        };
        let label = std::fs::read_to_string(dir.join("type"))
            .map_or_else(|_| "Sensor".to_owned(), |s| pretty(s.trim()));
        zones.push(ThermalZone {
            label,
            celsius: millideg as f64 / 1000.0,
        });
    }
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
