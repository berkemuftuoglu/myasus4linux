use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{
    battery,
    cpu::{CoreStat, CpuMonitor},
    detect::HardwareFeatures,
    error::BackendError,
    fan::{self, FanProfile},
    safeguards, thermal,
};
use crate::format::duration_hm;
use crate::ui::palette::{self, Rgb};
use crate::ui::widgets::battery_cell::BatteryCell;
use crate::ui::widgets::chart::Chart;
use crate::ui::widgets::gauge::{Accent, Gauge};
use crate::ui::widgets::ledbar::LedBar;
use crate::ui::widgets::meter::Meter;
use crate::ui::widgets::panel::Panel;
use crate::ui::widgets::stat::Stat;

pub struct Overview {
    monitor: Option<CpuMonitor>,
    has_battery: bool,
    low_batt_warned: bool,
    thermal_warned: bool,
    profile: crate::ui::commit::OptimisticChoice<FanProfile>,
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
}

#[derive(Debug)]
pub enum OverviewInput {
    Tick,
    Sampled(Box<OverviewSample>),
    SetMode(FanProfile),
    ModeWritten(Result<(), BackendError>),
}

/// Plain data carried back from the worker thread; no GTK, no borrowed state.
#[derive(Debug)]
pub struct OverviewSample {
    monitor: CpuMonitor,
    cores: Vec<CoreStat>,
    cpu_temp: Option<f64>,
    zones: Vec<thermal::ThermalZone>,
    battery: Option<battery::BatteryInfo>,
    profile: Option<FanProfile>,
}

#[relm4::component(pub)]
impl SimpleComponent for Overview {
    type Init = HardwareFeatures;
    type Input = OverviewInput;
    type Output = crate::ui::PageMsg;

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
                            set_active: model.profile.current() == FanProfile::Quiet,
                            connect_clicked[sender] => move |b| if b.is_active() {
                                sender.input(OverviewInput::SetMode(FanProfile::Quiet));
                            },
                        },
                        #[name = "mode_balanced"]
                        gtk::ToggleButton {
                            set_label: "Balanced",
                            #[watch]
                            set_active: model.profile.current() == FanProfile::Balanced,
                            connect_clicked[sender] => move |b| if b.is_active() {
                                sender.input(OverviewInput::SetMode(FanProfile::Balanced));
                            },
                        },
                        #[name = "mode_perf"]
                        gtk::ToggleButton {
                            set_label: "Performance",
                            #[watch]
                            set_active: model.profile.current() == FanProfile::Performance,
                            connect_clicked[sender] => move |b| if b.is_active() {
                                sender.input(OverviewInput::SetMode(FanProfile::Performance));
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        features: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut monitor = CpuMonitor::new();
        let cores = monitor.sample();

        let mut model = Overview {
            monitor: Some(monitor),
            has_battery: features.battery,
            low_batt_warned: false,
            thermal_warned: false,
            profile: crate::ui::commit::OptimisticChoice::new(FanProfile::Balanced),
            load_g: Gauge::new(168, Accent::ByValue),
            temp_g: Gauge::new(168, Accent::ByValue),
            batt_cell: BatteryCell::new(168),
            power_s: Stat::with_spark("Power Draw", Rgb::new(0.4, 0.7, 1.0)),
            freq_s: Stat::with_spark("Avg Frequency", palette::GOOD),
            time_s: Stat::new("Time Remaining"),
            wear_s: Stat::new("Battery Wear"),
            temp_chart: Chart::new(130, 110.0, Rgb::new(1.0, 0.6, 0.2), "°"),
            load_chart: Chart::new(130, 100.0, palette::GOOD, "%"),
            core_panel: Panel::new("Per-core Load"),
            core_leds: Vec::new(),
            zone_panel: Panel::new("Thermal Sensors"),
            zone_meters: Vec::new(),
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
        if model.has_battery {
            widgets.heroes.insert(
                &Panel::metric("Battery", &model.batt_cell.area, 240, true),
                -1,
            );
        }

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

        model.core_panel.root.set_size_request(320, -1);
        crate::ui::builders::cores::build(&model.core_panel, &mut model.core_leds, &cores);
        widgets.matrices.insert(&model.core_panel.root, -1);

        model.zone_panel.root.set_size_request(320, -1);
        crate::ui::builders::zones::build(&model.zone_panel, &mut model.zone_meters);
        widgets.matrices.insert(&model.zone_panel.root, -1);

        widgets.mode_balanced.set_group(Some(&widgets.mode_quiet));
        widgets.mode_perf.set_group(Some(&widgets.mode_quiet));

        sender.input(OverviewInput::Tick);
        crate::ui::poll(&root, sender.input_sender(), || OverviewInput::Tick);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            OverviewInput::Tick => {
                if let Some(mut monitor) = self.monitor.take() {
                    let has_battery = self.has_battery;
                    crate::ui::offload(sender.input_sender(), move || {
                        let cores = monitor.sample();
                        // A failed live read on the always-open dashboard is logged
                        // rather than silently dropped, but kept off the toast
                        // overlay so a flaky sensor can't spam a warning every poll.
                        let battery = if has_battery {
                            battery::read_battery_info()
                                .inspect_err(|e| tracing::warn!("overview battery read: {e}"))
                                .ok()
                        } else {
                            None
                        };
                        let profile = fan::read_profile()
                            .inspect_err(|e| tracing::warn!("overview profile read: {e}"))
                            .ok();
                        OverviewInput::Sampled(Box::new(OverviewSample {
                            cores,
                            cpu_temp: fan::read_cpu_temp(),
                            zones: thermal::read_zones(),
                            battery,
                            profile,
                            monitor,
                        }))
                    });
                }
            }
            OverviewInput::Sampled(sample) => self.show_sample(*sample, &sender),
            OverviewInput::SetMode(profile) => {
                if let Some(p) = self.profile.pick(profile) {
                    crate::ui::offload(sender.input_sender(), move || {
                        OverviewInput::ModeWritten(fan::set_profile(p))
                    });
                }
            }
            OverviewInput::ModeWritten(result) => {
                self.profile.written(result.is_ok());
                if let Err(e) = result {
                    let _ = sender.output(crate::ui::PageMsg::Error(e.to_string()));
                }
            }
        }
    }
}

impl Overview {
    /// Apply a worker-thread sample to the dashboard widgets and run the safety
    /// policies. Runs on the GTK main thread, so touching widgets is fine here.
    fn show_sample(&mut self, sample: OverviewSample, sender: &ComponentSender<Self>) {
        let OverviewSample {
            monitor,
            cores,
            cpu_temp,
            zones,
            battery,
            profile,
        } = sample;
        self.monitor = Some(monitor);
        // Adopt the freshly-read profile so the safeguards reason about the
        // current mode; poll() ignores it while our own write is in flight, so a
        // stale read can't stomp the optimistic value and bounce the toggles.
        if let Some(profile) = profile {
            self.profile.poll(profile);
        }

        let n = cores.len().max(1) as f64;
        let load = cores.iter().map(|c| c.load).sum::<f64>() / n;
        let freq = f64::from(cores.iter().map(|c| c.mhz).sum::<u32>()) / n / 1000.0;
        self.load_g
            .set(load / 100.0, &format!("{load:.0}%"), "Load");
        self.load_chart.push(load);
        self.freq_s.set(&format!("{freq:.1}"), "GHz, all cores");
        self.freq_s.push(freq);
        crate::ui::builders::cores::update(&self.core_leds, &cores);
        self.core_panel.set_corner(&format!("avg {load:.0}%"));

        if let Some(t) = cpu_temp {
            self.temp_g.set(t / 100.0, &format!("{t:.0}°"), "CPU");
            self.temp_chart.push(t);
        }

        crate::ui::builders::zones::update(&self.zone_meters, &zones);
        if let Some(hot) = zones.iter().max_by(|a, b| a.celsius.total_cmp(&b.celsius)) {
            self.zone_panel
                .set_corner(&format!("max {:.0}°C", hot.celsius));
            // Safeguard feedback: the daemon's thermal guard forces maximum
            // cooling headless and restores the profile when it cools; here we
            // just tell the user, once per hot episode (reset once it cools), so
            // there aren't two loops fighting over the same sysfs node. Name the
            // zone so the gauge's CPU temp not matching doesn't look like a glitch.
            if safeguards::thermal_override(hot.celsius, self.profile.current()).is_some() {
                if !self.thermal_warned {
                    self.thermal_warned = true;
                    let _ = sender.output(crate::ui::PageMsg::Notice(format!(
                        "{} at {:.0}°C is too hot, forcing Performance to cool down",
                        hot.label, hot.celsius
                    )));
                }
            } else if hot.celsius < safeguards::THERMAL_LIMIT_C {
                self.thermal_warned = false;
            }
        }

        if let Some(b) = battery {
            self.show_battery(&b, sender);
        }
    }

    fn show_battery(&mut self, b: &battery::BatteryInfo, sender: &ComponentSender<Self>) {
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

        // Safeguard: nudge toward quiet mode when low and unplugged, once per
        // low-battery episode so it doesn't nag every tick. Prefer the actual AC
        // state over "charging" -- a full battery on AC isn't charging but also
        // isn't running down, so it shouldn't trigger the nudge.
        let plugged = b.on_ac.unwrap_or(charging);
        if safeguards::suggest_quiet(cap, plugged, self.profile.current()) {
            if !self.low_batt_warned {
                self.low_batt_warned = true;
                let _ = sender.output(crate::ui::PageMsg::Notice(
                    "Battery low, switch to Quiet mode to save power".to_owned(),
                ));
            }
        } else if plugged || cap > safeguards::LOW_BATTERY_PCT {
            self.low_batt_warned = false;
        }
    }
}
