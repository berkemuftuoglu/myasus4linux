//! Thermal-sensor meters, shared by the overview and cooling dashboards.

use gtk::prelude::*;

use super::meter::Meter;
use super::panel::Panel;
use crate::backend::thermal::{self, ThermalZone};

/// Build one meter per thermal zone (sorted by label), append them to `panel`,
/// and pair each with its label so later reads can match by name.
pub fn build(panel: &Panel, meters: &mut Vec<(String, Meter)>) {
    panel.body.set_spacing(7);
    let mut zones = thermal::read_zones();
    zones.sort_by(|a, b| a.label.cmp(&b.label));
    for zone in zones {
        let meter = Meter::new(&zone.label);
        panel.body.append(&meter.root);
        meters.push((zone.label, meter));
    }
}

/// Push the latest readings into the meters, matched by label.
pub fn update(meters: &[(String, Meter)], zones: &[ThermalZone]) {
    for (label, meter) in meters {
        if let Some(zone) = zones.iter().find(|z| &z.label == label) {
            meter.set(zone.celsius / 100.0, &format!("{:.0}°C", zone.celsius));
        }
    }
}
