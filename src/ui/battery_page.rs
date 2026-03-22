use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{battery, error::BackendError};

/// Battery page component showing charge info and charge limit control.
pub struct BatteryPage {
    capacity: u8,
    status: String,
    health_percent: f64,
    cycle_count: Option<u32>,
    charge_threshold: u8,
}

#[derive(Debug)]
pub enum BatteryInput {
    LoadValues,
    ValuesLoaded(Box<battery::BatteryInfo>),
    SetChargeThreshold(u8),
    ThresholdWritten(Result<(), BackendError>),
    ReadError(String),
}

#[derive(Debug)]
pub enum BatteryOutput {
    Error(String),
}

#[relm4::component(pub)]
impl SimpleComponent for BatteryPage {
    type Init = ();
    type Input = BatteryInput;
    type Output = BatteryOutput;

    view! {
        adw::PreferencesPage {
            set_title: "Battery",
            set_icon_name: Some("battery-symbolic"),

            adw::PreferencesGroup {
                set_title: "Status",

                adw::ActionRow {
                    set_title: "Charge Level",
                    #[watch]
                    set_subtitle: &format!("{}%", model.capacity),
                },

                adw::ActionRow {
                    set_title: "Status",
                    #[watch]
                    set_subtitle: &model.status,
                },

                adw::ActionRow {
                    set_title: "Health",
                    #[watch]
                    set_subtitle: &format!(
                        "{:.1}% ({})",
                        model.health_percent,
                        battery::health_label(model.health_percent),
                    ),
                },

                adw::ActionRow {
                    set_title: "Cycle Count",
                    #[watch]
                    set_subtitle: &model.cycle_count
                        .map_or_else(|| "Unknown".to_owned(), |c| c.to_string()),
                },
            },

            adw::PreferencesGroup {
                set_title: "Charge Limit",
                set_description: Some(
                    "Limiting the maximum charge extends battery lifespan. \
                     Recommended: 80%.",
                ),

                adw::SpinRow {
                    set_title: "End Threshold",
                    set_subtitle: "Stop charging at this percentage",
                    #[watch]
                    set_value: f64::from(model.charge_threshold),
                    set_adjustment: Some(&gtk::Adjustment::new(
                        80.0,   // default
                        40.0,   // min (safeguard: never below 40)
                        100.0,  // max
                        1.0,    // step
                        5.0,    // page step
                        0.0,    // page size
                    )),
                    connect_value_notify[sender] => move |row| {
                        sender.input(BatteryInput::SetChargeThreshold(row.value() as u8));
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
        let model = BatteryPage {
            capacity: 0,
            status: "Loading...".to_owned(),
            health_percent: 100.0,
            cycle_count: None,
            charge_threshold: 80,
        };

        let widgets = view_output!();
        sender.input(BatteryInput::LoadValues);
        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: Self::Input,
        sender: ComponentSender<Self>,
    ) {
        match msg {
            BatteryInput::LoadValues => {
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    let msg = match battery::read_battery_info() {
                        Ok(info) => BatteryInput::ValuesLoaded(Box::new(info)),
                        Err(e) => BatteryInput::ReadError(e.to_string()),
                    };
                    input_sender.send(msg);
                });
            }
            BatteryInput::ValuesLoaded(info) => {
                self.capacity = info.capacity;
                self.status = info.status;
                self.health_percent = info.health_percent;
                self.cycle_count = info.cycle_count;
                if let Some(threshold) = info.charge_threshold {
                    self.charge_threshold = threshold;
                }
            }
            BatteryInput::SetChargeThreshold(val) => {
                self.charge_threshold = val;
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    input_sender.send(BatteryInput::ThresholdWritten(
                        battery::set_charge_threshold(val),
                    ));
                });
            }
            BatteryInput::ThresholdWritten(result) => {
                if let Err(e) = result {
                    let _ = sender.output(BatteryOutput::Error(e.to_string()));
                }
            }
            BatteryInput::ReadError(msg) => {
                self.status = "Error".to_owned();
                let _ = sender.output(BatteryOutput::Error(msg));
            }
        }
    }
}
