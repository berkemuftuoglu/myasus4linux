use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{battery, error::BackendError};
use crate::format::duration_hm;
use crate::ui::palette::Rgb;
use crate::ui::widgets::battery_cell::BatteryCell;
use crate::ui::widgets::gauge::{Accent, Gauge};
use crate::ui::widgets::panel::Panel;
use crate::ui::widgets::stat::Stat;

pub struct BatteryPage {
    charge: crate::ui::commit::DebouncedCommit<u8>,
    charging: bool,
    charge_supported: bool,
    charge_cell: BatteryCell,
    health_g: Gauge,
    power_s: Stat,
    volt_s: Stat,
    curr_s: Stat,
    time_s: Stat,
    cycle_s: Stat,
    src_s: Stat,
}

#[derive(Debug)]
pub enum BatteryInput {
    LoadValues,
    ValuesLoaded(Box<battery::BatteryInfo>),
    SliderMoved(u8),
    CommitThreshold(u32),
    ThresholdWritten(Result<(), BackendError>),
    ReadError(String),
}

impl BatteryPage {
    /// Push a fresh battery reading into all the widgets.
    fn show_values(&mut self, info: &battery::BatteryInfo) {
        self.charging = info.is_charging();
        let flow_label = if self.charging {
            "watts in"
        } else {
            "watts out"
        };

        let cap = info.capacity;
        self.charge_cell.set(
            f64::from(cap) / 100.0,
            self.charging,
            &format!("{cap}%"),
            info.status.label(),
        );

        let health = info.health_percent;
        self.health_g.set(
            health / 100.0,
            &format!("{health:.0}%"),
            battery::HealthStatus::from_percent(health).label(),
        );

        if let Some(w) = info.power_w {
            self.power_s.set(&format!("{w:.1}"), flow_label);
            self.power_s.push(w);
        } else {
            self.power_s.set("—", "watts");
        }
        if let Some(v) = info.voltage_mv {
            let volts = f64::from(v) / 1000.0;
            self.volt_s.set(&format!("{volts:.2}"), "volts");
            self.volt_s.push(volts);
        } else {
            self.volt_s.set("—", "volts");
        }
        if let Some(a) = info.current_ma {
            self.curr_s.set(&a.abs().to_string(), "milliamps");
            self.curr_s.push(f64::from(a.abs()));
        } else {
            self.curr_s.set("—", "milliamps");
        }
        self.time_s.set(
            &duration_hm(info.time_remaining_h),
            if self.charging {
                "until full"
            } else {
                "until empty"
            },
        );
        self.cycle_s.set(
            &info
                .cycle_count
                .map_or_else(|| "—".to_owned(), |c| c.to_string()),
            "charge cycles",
        );

        let (src, src_sub) = match info.on_ac {
            Some(true) => ("AC", "plugged in"),
            Some(false) => ("Battery", "on battery"),
            None => ("—", "power source"),
        };
        self.src_s.set(src, src_sub);

        // poll() ignores this while a write is in flight, so it can't stomp the
        // optimistic value mid-drag.
        if let Some(threshold) = info.charge_threshold {
            self.charge.poll(threshold);
        }
    }
}

#[relm4::component(pub)]
impl SimpleComponent for BatteryPage {
    type Init = bool;
    type Input = BatteryInput;
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
                    set_label: "Battery",
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

                gtk::Box {
                    add_css_class: "panel",
                    set_orientation: gtk::Orientation::Vertical,
                    set_visible: model.charge_supported,

                    gtk::Box {
                        add_css_class: "panel-header",
                        gtk::Label {
                            set_label: "Charge Limit",
                            add_css_class: "panel-title",
                            set_halign: gtk::Align::Start,
                            set_hexpand: true,
                        },
                        gtk::Label {
                            add_css_class: "panel-corner",
                            #[watch]
                            set_label: &format!("{}%", model.charge.value()),
                        },
                    },

                    gtk::Box {
                        add_css_class: "panel-body",
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            add_css_class: "dim-label",
                            set_label: "Stop charging at this level. 80% is recommended to extend lifespan.",
                        },

                        #[name = "limit_scale"]
                        gtk::Scale {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_hexpand: true,
                            set_draw_value: false,
                            set_round_digits: 0,
                            set_adjustment: &gtk::Adjustment::new(
                                f64::from(battery::THRESHOLD_DEFAULT),
                                f64::from(battery::THRESHOLD_MIN),
                                f64::from(battery::THRESHOLD_MAX),
                                1.0, 5.0, 0.0,
                            ),
                            #[watch]
                            #[block_signal(limit_changed)]
                            set_value: f64::from(model.charge.value()),
                            connect_value_changed[sender] => move |s| {
                                sender.input(BatteryInput::SliderMoved(
                                    crate::num::round_u8_in(s.value(), battery::THRESHOLD_MIN, battery::THRESHOLD_MAX),
                                ));
                            } @limit_changed,
                        },

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            add_css_class: "warning",
                            set_label: "Keeping the battery at 100% shortens its lifespan.",
                            #[watch]
                            set_visible: model.charge.value() >= 100,
                        },
                    },
                },
            },
        }
    }

    fn init(
        charge_supported: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let initial = battery::charge_threshold().unwrap_or(battery::THRESHOLD_DEFAULT);
        let model = BatteryPage {
            charge: crate::ui::commit::DebouncedCommit::new(initial),
            charging: false,
            charge_supported,
            charge_cell: BatteryCell::new(168),
            health_g: Gauge::new(168, Accent::Fixed(Rgb::new(0.4, 0.7, 1.0))),
            power_s: Stat::with_spark("Power Flow", Rgb::new(0.4, 0.7, 1.0)),
            volt_s: Stat::with_spark("Voltage", Rgb::new(0.6, 0.5, 1.0)),
            curr_s: Stat::with_spark("Current", Rgb::new(1.0, 0.6, 0.2)),
            time_s: Stat::new("Time Remaining"),
            cycle_s: Stat::new("Cycle Count"),
            src_s: Stat::new("Power Source"),
        };

        let widgets = view_output!();

        widgets.heroes.insert(
            &Panel::metric("Charge Level", &model.charge_cell.area, 240, true),
            -1,
        );
        widgets.heroes.insert(
            &Panel::metric("Health", &model.health_g.area, 240, true),
            -1,
        );
        model.power_s.root.set_size_request(200, -1);
        widgets.heroes.insert(&model.power_s.root, -1);

        for s in [
            &model.volt_s,
            &model.curr_s,
            &model.time_s,
            &model.cycle_s,
            &model.src_s,
        ] {
            s.root.set_size_request(168, -1);
            widgets.stats.insert(&s.root, -1);
        }

        for mark in [
            f64::from(battery::THRESHOLD_MIN),
            60.0,
            80.0,
            f64::from(battery::THRESHOLD_MAX),
        ] {
            widgets.limit_scale.add_mark(
                mark,
                gtk::PositionType::Bottom,
                Some(&format!("{mark:.0}")),
            );
        }

        sender.input(BatteryInput::LoadValues);
        crate::ui::poll(&root, sender.input_sender(), || BatteryInput::LoadValues);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            BatteryInput::LoadValues => {
                crate::ui::offload(sender.input_sender(), || {
                    match battery::read_battery_info() {
                        Ok(info) => BatteryInput::ValuesLoaded(Box::new(info)),
                        Err(e) => BatteryInput::ReadError(e.to_string()),
                    }
                });
            }
            BatteryInput::ValuesLoaded(info) => self.show_values(&info),
            BatteryInput::SliderMoved(val) => {
                // Optimistic display now; defer the privileged write until the
                // drag settles, so one drag is one authorised write, not ~40.
                if let Some(seq) = self.charge.slide(val) {
                    crate::ui::debounce_commit(
                        sender.input_sender(),
                        seq,
                        BatteryInput::CommitThreshold,
                    );
                }
            }
            BatteryInput::CommitThreshold(seq) => {
                if let Some(val) = self.charge.commit(seq) {
                    crate::ui::offload(sender.input_sender(), move || {
                        BatteryInput::ThresholdWritten(battery::set_charge_threshold(val))
                    });
                }
            }
            BatteryInput::ThresholdWritten(result) => {
                // Sliders update visibly and live, so they commit quietly; only a
                // failure is worth a toast. (Discrete buttons still confirm.)
                self.charge.written(result.is_ok());
                if let Err(e) = result {
                    let _ = sender.output(crate::ui::PageMsg::Error(e.to_string()));
                }
            }
            BatteryInput::ReadError(msg) => {
                let _ = sender.output(crate::ui::PageMsg::Error(msg));
            }
        }
    }
}
