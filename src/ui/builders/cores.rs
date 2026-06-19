//! Per-core load and frequency LED bars, shared by the CPU and overview
//! dashboards so both render the core matrix the same way.

use gtk::prelude::*;

use crate::ui::widgets::ledbar::LedBar;
use crate::ui::widgets::panel::Panel;
use crate::backend::cpu::CoreStat;

/// Fill `panel` with one LED bar per core and collect the bars for later updates.
pub fn build(panel: &Panel, leds: &mut Vec<LedBar>, cores: &[CoreStat]) {
    panel.body.set_orientation(gtk::Orientation::Vertical);
    panel.body.set_spacing(7);
    for core in cores {
        let led = LedBar::new(&format!("Core {}", core.id));
        panel.body.append(&led.root);
        leds.push(led);
    }
}

/// Push the latest per-core load and frequency into the existing bars.
pub fn update(leds: &[LedBar], cores: &[CoreStat]) {
    for (core, led) in cores.iter().zip(leds) {
        led.set(
            core.load / 100.0,
            &format!("{:.1}GHz", f64::from(core.mhz) / 1000.0),
        );
    }
}
