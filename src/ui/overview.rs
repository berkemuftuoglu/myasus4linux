use adw::prelude::*;
use relm4::prelude::*;

use super::battery_cell::BatteryCell;
use super::chart::Chart;
use super::gauge::{Accent, Gauge};
use super::ledbar::LedBar;
use super::meter::Meter;
use super::panel::Panel;
use super::stat::Stat;
use crate::backend::{
    battery,
    cpu::CpuMonitor,
    fan::{self, FanProfile},
    thermal,
};
use crate::format::duration_hm;
use crate::palette::{self, Rgb};

pub struct Overview {
    load_g: Gauge,
    temp_g: Gauge,
    batt_cell: BatteryCell,
    power_s: Stat,
    freq_s: Stat,
    time_s: Stat,
    wear_s: Stat,
    temp_chart: Chart,
    load_chart: Chart,
    core_panel: Panel,
    core_leds: Vec<LedBar>,
    zone_panel: Panel,
    zone_meters: Vec<(String, Meter)>,
    monitor: CpuMonitor,
    active_mode: usize,
}

#[derive(Debug)]
pub enum OverviewInput {
    Tick,
    SetMode(usize),
}

#[derive(Debug)]
pub enum OverviewOutput {
    Error(String),
}

#[relm4::component(pub)]
impl SimpleComponent for Overview {
    type Init = ();
    type Input = OverviewInput;
    type Output = OverviewOutput;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 16,
                set_margin_top: 22,
                set_margin_bottom: 22,
                set_margin_start: 22,
                set_margin_end: 22,

                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_label: "Overview",
                    add_css_class: "title-1",
                },

                #[name = "heroes"]
                gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_column_spacing: 14,
                    set_row_spacing: 14,
                    set_min_children_per_line: 1,
                    set_max_children_per_line: 3,
                },

                #[name = "stats"]
                gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_column_spacing: 14,
                    set_row_spacing: 14,
                    set_min_children_per_line: 2,
                    set_max_children_per_line: 4,
                },

                #[name = "trends"]
                gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_column_spacing: 14,
                    set_row_spacing: 14,
                    set_min_children_per_line: 1,
                    set_max_children_per_line: 2,
                },

                #[name = "matrices"]
                gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_column_spacing: 14,
                    set_row_spacing: 14,
                    set_min_children_per_line: 1,
                    set_max_children_per_line: 2,
                },

                gtk::Box {
                    add_css_class: "panel",
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Box {
                        add_css_class: "panel-header",
                        gtk::Label {
                            set_label: "Power Mode",
                            add_css_class: "panel-title",
                            set_halign: gtk::Align::Start,
                            set_hexpand: true,
                        },
                    },

                    gtk::Box {
                        add_css_class: "panel-body",
                        set_homogeneous: true,
                        add_css_class: "linked",

                        #[name = "mode_quiet"]
                        gtk::ToggleButton {
                            set_label: "Quiet",
                            #[watch]
                            set_active: model.active_mode == 0,
                            connect_clicked[sender] => move |b| if b.is_active() {
                                sender.input(OverviewInput::SetMode(0));
                            },
                        },
                        #[name = "mode_balanced"]
                        gtk::ToggleButton {
                            set_label: "Balanced",
                            #[watch]
                            set_active: model.active_mode == 1,
                            connect_clicked[sender] => move |b| if b.is_active() {
                                sender.input(OverviewInput::SetMode(1));
                            },
                        },
                        #[name = "mode_perf"]
                        gtk::ToggleButton {
                            set_label: "Performance",
                            #[watch]
                            set_active: model.active_mode == 2,
                            connect_clicked[sender] => move |b| if b.is_active() {
                                sender.input(OverviewInput::SetMode(2));
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = Overview {
            load_g: Gauge::new(168, Accent::ByValue),
            temp_g: Gauge::new(168, Accent::ByValue),
            batt_cell: BatteryCell::new(168),
            power_s: Stat::with_spark("Power Draw", Rgb::new(0.4, 0.7, 1.0)),
            freq_s: Stat::with_spark("Avg Frequency", palette::GOOD),
            time_s: Stat::new("Time Remaining"),
            wear_s: Stat::new("Battery Wear"),
            temp_chart: Chart::new(130, 100.0, Rgb::new(1.0, 0.6, 0.2), "°"),
            load_chart: Chart::new(130, 100.0, palette::GOOD, "%"),
            core_panel: Panel::new("Per-core Load"),
            core_leds: Vec::new(),
            zone_panel: Panel::new("Thermal Sensors"),
            zone_meters: Vec::new(),
            monitor: CpuMonitor::new(),
            active_mode: 1,
        };

        let widgets = view_output!();

        widgets.heroes.insert(
            &Panel::metric("CPU Load", &model.load_g.area, 240, true),
            -1,
        );
        widgets.heroes.insert(
            &Panel::metric("CPU Temperature", &model.temp_g.area, 240, true),
            -1,
        );
        widgets.heroes.insert(
            &Panel::metric("Battery", &model.batt_cell.area, 240, true),
            -1,
        );

        for s in [&model.power_s, &model.freq_s, &model.time_s, &model.wear_s] {
            s.root.set_size_request(168, -1);
            widgets.stats.insert(&s.root, -1);
        }

        widgets.trends.insert(
            &Panel::metric("CPU Temperature", &model.temp_chart.area, 340, false),
            -1,
        );
        widgets.trends.insert(
            &Panel::metric("CPU Load", &model.load_chart.area, 340, false),
            -1,
        );

        model
            .core_panel
            .body
            .set_orientation(gtk::Orientation::Vertical);
        model.core_panel.body.set_spacing(7);
        model.core_panel.root.set_size_request(320, -1);
        for core in model.monitor.sample() {
            let led = LedBar::new(&format!("Core {}", core.id));
            model.core_panel.body.append(&led.root);
            model.core_leds.push(led);
        }
        widgets.matrices.insert(&model.core_panel.root, -1);

        model
            .zone_panel
            .body
            .set_orientation(gtk::Orientation::Vertical);
        model.zone_panel.body.set_spacing(7);
        model.zone_panel.root.set_size_request(320, -1);
        let mut zones = thermal::read_zones();
        zones.sort_by(|a, b| a.label.cmp(&b.label));
        for zone in zones {
            let meter = Meter::new(&zone.label);
            model.zone_panel.body.append(&meter.root);
            model.zone_meters.push((zone.label, meter));
        }
        widgets.matrices.insert(&model.zone_panel.root, -1);

        widgets.mode_balanced.set_group(Some(&widgets.mode_quiet));
        widgets.mode_perf.set_group(Some(&widgets.mode_quiet));

        sender.input(OverviewInput::Tick);
        let ticker = sender.clone();
        glib::timeout_add_seconds_local(1, move || {
            ticker.input(OverviewInput::Tick);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            OverviewInput::Tick => {
                let cores = self.monitor.sample();
                let n = cores.len().max(1) as f64;
                let load = cores.iter().map(|c| c.load).sum::<f64>() / n;
                let freq = f64::from(cores.iter().map(|c| c.mhz).sum::<u32>()) / n / 1000.0;

                self.load_g
                    .set(load / 100.0, &format!("{load:.0}%"), "Load");
                self.load_chart.push(load);
                self.freq_s.set(&format!("{freq:.1}"), "GHz, all cores");
                self.freq_s.push(freq);

                for (core, led) in cores.iter().zip(&self.core_leds) {
                    led.set(
                        core.load / 100.0,
                        &format!("{:.1}GHz", f64::from(core.mhz) / 1000.0),
                    );
                }
                self.core_panel.set_corner(&format!("avg {load:.0}%"));

                if let Some(t) = fan::read_cpu_temp() {
                    self.temp_g.set(t / 100.0, &format!("{t:.0}°"), "CPU");
                    self.temp_chart.push(t);
                }

                let zones = thermal::read_zones();
                for (label, meter) in &self.zone_meters {
                    if let Some(zone) = zones.iter().find(|z| &z.label == label) {
                        meter.set(zone.celsius / 100.0, &format!("{:.0}°C", zone.celsius));
                    }
                }
                if let Some(hottest) = zones.iter().map(|z| z.celsius).reduce(f64::max) {
                    self.zone_panel.set_corner(&format!("max {hottest:.0}°C"));
                }

                if let Ok(b) = battery::read_battery_info() {
                    let cap = b.capacity;
                    let charging = b.is_charging();
                    self.batt_cell.set(
                        f64::from(cap) / 100.0,
                        charging,
                        &format!("{cap}%"),
                        if charging { "charging" } else { "battery" },
                    );
                    if let Some(w) = b.power_w {
                        self.power_s.set(&format!("{w:.1}"), "watts, current flow");
                        self.power_s.push(w);
                    } else {
                        self.power_s.set("—", "watts");
                    }
                    self.time_s.set(
                        &duration_hm(b.time_remaining_h),
                        if charging {
                            "until full"
                        } else {
                            "until empty"
                        },
                    );
                    self.wear_s.set(
                        &format!("{:.0}%", (100.0 - b.health_percent).max(0.0)),
                        "capacity lost",
                    );
                }

                self.active_mode = match fan::read_profile().unwrap_or(FanProfile::Balanced) {
                    FanProfile::Quiet => 0,
                    FanProfile::Balanced => 1,
                    FanProfile::Performance => 2,
                };
            }
            OverviewInput::SetMode(index) => {
                self.active_mode = index;
                let profile = match index {
                    0 => FanProfile::Quiet,
                    2 => FanProfile::Performance,
                    _ => FanProfile::Balanced,
                };
                let out = sender.output_sender().clone();
                std::thread::spawn(move || {
                    if let Err(e) = fan::set_profile(profile) {
                        let _ = out.send(OverviewOutput::Error(e.to_string()));
                    }
                });
            }
        }
    }
}
