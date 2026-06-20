use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{
    error::BackendError,
    fan::{self, FanProfile},
    thermal,
};
use crate::ui::widgets::gauge::{Accent, Gauge};
use crate::ui::widgets::meter::Meter;
use crate::ui::widgets::panel::Panel;
use crate::ui::widgets::stat::Stat;

pub struct FanPage {
    profile: crate::ui::commit::OptimisticChoice<FanProfile>,
    temp_g: Gauge,
    fan_stats: Vec<(String, Stat)>,
    zone_meters: Vec<(String, Meter)>,
}

#[derive(Debug)]
pub enum FanInput {
    Tick,
    Loaded(Box<FanSample>),
    SetProfile(FanProfile),
    ProfileWritten(Result<(), BackendError>),
    ReadError(String),
}

/// Plain data carried back from the worker thread.
#[derive(Debug)]
pub struct FanSample {
    profile: FanProfile,
    temp: Option<f64>,
    fans: Vec<fan::FanReading>,
    zones: Vec<thermal::ThermalZone>,
}

#[relm4::component(pub)]
impl SimpleComponent for FanPage {
    type Init = ();
    type Input = FanInput;
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
                                set_active: model.profile.current() == FanProfile::Quiet,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(FanInput::SetProfile(FanProfile::Quiet));
                                },
                            },
                            #[name = "mode_balanced"]
                            gtk::ToggleButton {
                                set_label: "Balanced",
                                #[watch]
                                set_active: model.profile.current() == FanProfile::Balanced,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(FanInput::SetProfile(FanProfile::Balanced));
                                },
                            },
                            #[name = "mode_perf"]
                            gtk::ToggleButton {
                                set_label: "Performance",
                                #[watch]
                                set_active: model.profile.current() == FanProfile::Performance,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(FanInput::SetProfile(FanProfile::Performance));
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
            profile: crate::ui::commit::OptimisticChoice::new(FanProfile::Balanced),
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
        crate::ui::builders::zones::build(&sensors, &mut model.zone_meters);
        widgets.sensors_slot.append(&sensors.root);

        sender.input(FanInput::Tick);
        crate::ui::poll(&root, sender.input_sender(), || FanInput::Tick);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            FanInput::Tick => {
                crate::ui::offload(sender.input_sender(), || match fan::read_profile() {
                    Ok(profile) => FanInput::Loaded(Box::new(FanSample {
                        profile,
                        temp: fan::read_cpu_temp(),
                        fans: fan::read_fans(),
                        zones: thermal::read_zones(),
                    })),
                    Err(e) => FanInput::ReadError(e.to_string()),
                });
            }
            FanInput::Loaded(sample) => {
                let FanSample {
                    profile,
                    temp,
                    fans,
                    zones,
                } = *sample;
                self.profile.poll(profile);
                if let Some(t) = temp {
                    self.temp_g
                        .set(t / 100.0, &format!("{t:.0}°"), "CPU package");
                }
                for (label, stat) in &self.fan_stats {
                    if let Some(reading) = fans.iter().find(|f| &f.label == label) {
                        stat.set(&reading.rpm.to_string(), "RPM");
                    }
                }
                crate::ui::builders::zones::update(&self.zone_meters, &zones);
            }
            FanInput::SetProfile(profile) => {
                if let Some(p) = self.profile.pick(profile) {
                    crate::ui::offload(sender.input_sender(), move || {
                        FanInput::ProfileWritten(fan::set_profile(p))
                    });
                }
            }
            FanInput::ProfileWritten(result) => {
                self.profile.written(result.is_ok());
                match result {
                    // A discrete control, so confirm it (unlike the sliders).
                    Ok(()) => {
                        let _ = sender.output(crate::ui::PageMsg::Notice(format!(
                            "{} mode applied",
                            self.profile.current().label()
                        )));
                    }
                    Err(e) => {
                        let _ = sender.output(crate::ui::PageMsg::Error(e.to_string()));
                    }
                }
            }
            FanInput::ReadError(msg) => {
                let _ = sender.output(crate::ui::PageMsg::Error(msg));
            }
        }
    }
}
