use adw::prelude::*;
use relm4::prelude::*;

use super::gauge::{Accent, Gauge};
use super::meter::Meter;
use super::panel::Panel;
use super::stat::Stat;
use crate::backend::{
    error::BackendError,
    fan::{self, FanProfile},
    thermal,
};

pub struct FanPage {
    current_profile: FanProfile,
    temp_g: Gauge,
    fan_stats: Vec<(String, Stat)>,
    zone_meters: Vec<(String, Meter)>,
}

#[derive(Debug)]
pub enum FanInput {
    Tick,
    Loaded(FanProfile, Option<f64>, Vec<fan::FanReading>),
    SetProfile(u8),
    ProfileWritten(Result<(), BackendError>),
    ReadError(String),
}

#[derive(Debug)]
pub enum FanOutput {
    Error(String),
}

#[relm4::component(pub)]
impl SimpleComponent for FanPage {
    type Init = ();
    type Input = FanInput;
    type Output = FanOutput;

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
                    set_label: "Cooling",
                    add_css_class: "title-1",
                },

                gtk::Box {
                    add_css_class: "panel",
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Box {
                        add_css_class: "panel-header",
                        gtk::Label {
                            set_label: "Thermal Profile",
                            add_css_class: "panel-title",
                            set_halign: gtk::Align::Start,
                            set_hexpand: true,
                        },
                    },

                    gtk::Box {
                        add_css_class: "panel-body",
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

                        gtk::Box {
                            set_homogeneous: true,
                            add_css_class: "linked",

                            #[name = "mode_quiet"]
                            gtk::ToggleButton {
                                set_label: "Quiet",
                                #[watch]
                                set_active: model.current_profile == FanProfile::Quiet,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(FanInput::SetProfile(FanProfile::Quiet as u8));
                                },
                            },
                            #[name = "mode_balanced"]
                            gtk::ToggleButton {
                                set_label: "Balanced",
                                #[watch]
                                set_active: model.current_profile == FanProfile::Balanced,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(FanInput::SetProfile(FanProfile::Balanced as u8));
                                },
                            },
                            #[name = "mode_perf"]
                            gtk::ToggleButton {
                                set_label: "Performance",
                                #[watch]
                                set_active: model.current_profile == FanProfile::Performance,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(FanInput::SetProfile(FanProfile::Performance as u8));
                                },
                            },
                        },

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            add_css_class: "dim-label",
                            set_label: "Performance raises the power limit and fan speed. Quiet does the opposite.",
                        },
                    },
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

                #[name = "sensors_slot"]
                gtk::Box {},
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = FanPage {
            current_profile: FanProfile::Balanced,
            temp_g: Gauge::new(150, Accent::ByValue),
            fan_stats: Vec::new(),
            zone_meters: Vec::new(),
        };

        let widgets = view_output!();

        widgets.mode_balanced.set_group(Some(&widgets.mode_quiet));
        widgets.mode_perf.set_group(Some(&widgets.mode_quiet));

        widgets.heroes.insert(
            &Panel::metric("CPU Temperature", &model.temp_g.area, 240, true),
            -1,
        );

        for f in fan::read_fans() {
            let stat = Stat::new(&f.label);
            stat.root.set_size_request(200, -1);
            widgets.heroes.insert(&stat.root, -1);
            model.fan_stats.push((f.label, stat));
        }
        if model.fan_stats.is_empty() {
            let stat = Stat::new("Fan Speed");
            stat.set("n/a", "no tachometer on this model");
            stat.root.set_size_request(200, -1);
            widgets.heroes.insert(&stat.root, -1);
        }

        let sensors = Panel::new("Thermal Sensors");
        sensors.body.set_orientation(gtk::Orientation::Vertical);
        sensors.body.set_spacing(8);
        let mut zones = thermal::read_zones();
        zones.sort_by(|a, b| a.label.cmp(&b.label));
        for zone in zones {
            let meter = Meter::new(&zone.label);
            sensors.body.append(&meter.root);
            model.zone_meters.push((zone.label, meter));
        }
        widgets.sensors_slot.append(&sensors.root);

        sender.input(FanInput::Tick);
        let ticker = sender.clone();
        glib::timeout_add_seconds_local(2, move || {
            ticker.input(FanInput::Tick);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            FanInput::Tick => {
                let zones = thermal::read_zones();
                for (label, meter) in &self.zone_meters {
                    if let Some(zone) = zones.iter().find(|z| &z.label == label) {
                        meter.set(zone.celsius / 100.0, &format!("{:.0}°C", zone.celsius));
                    }
                }
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    let msg = match fan::read_profile() {
                        Ok(profile) => {
                            FanInput::Loaded(profile, fan::read_cpu_temp(), fan::read_fans())
                        }
                        Err(e) => FanInput::ReadError(e.to_string()),
                    };
                    let _ = input_sender.send(msg);
                });
            }
            FanInput::Loaded(profile, temp, fans) => {
                self.current_profile = profile;
                if let Some(t) = temp {
                    self.temp_g
                        .set(t / 100.0, &format!("{t:.0}°"), "CPU package");
                }
                for (label, stat) in &self.fan_stats {
                    if let Some(reading) = fans.iter().find(|f| &f.label == label) {
                        stat.set(&reading.rpm.to_string(), "RPM");
                    }
                }
            }
            FanInput::SetProfile(raw) => {
                if let Ok(profile) = FanProfile::from_raw(raw) {
                    self.current_profile = profile;
                    let input_sender = sender.input_sender().clone();
                    std::thread::spawn(move || {
                        let _ =
                            input_sender.send(FanInput::ProfileWritten(fan::set_profile(profile)));
                    });
                }
            }
            FanInput::ProfileWritten(result) => {
                if let Err(e) = result {
                    let _ = sender.output(FanOutput::Error(e.to_string()));
                }
            }
            FanInput::ReadError(msg) => {
                let _ = sender.output(FanOutput::Error(msg));
            }
        }
    }
}
