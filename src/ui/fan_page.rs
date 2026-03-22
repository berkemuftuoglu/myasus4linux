use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{error::BackendError, fan::{self, FanProfile}};

/// Fan profile page component.
pub struct FanPage {
    current_profile: FanProfile,
}

#[derive(Debug)]
pub enum FanInput {
    LoadProfile,
    ProfileLoaded(FanProfile),
    SetProfile(u32),
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
        adw::PreferencesPage {
            set_title: "Fan",
            set_icon_name: Some("preferences-system-power-symbolic"),

            adw::PreferencesGroup {
                set_title: "Thermal Profile",
                set_description: Some(
                    "Controls fan speed and power limits. \
                     Performance mode allows higher TDP but louder fans.",
                ),

                adw::ComboRow {
                    set_title: "Profile",
                    set_subtitle: "Select fan and power profile",
                    set_model: Some(&gtk::StringList::new(&[
                        "Balanced",
                        "Performance",
                        "Quiet",
                    ])),
                    #[watch]
                    set_selected: model.current_profile.as_raw() as u32,
                    connect_selected_notify[sender] => move |row| {
                        sender.input(FanInput::SetProfile(row.selected()));
                    },
                },

                adw::ActionRow {
                    set_title: "Current Mode",
                    #[watch]
                    set_subtitle: model.current_profile.label(),
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = FanPage {
            current_profile: FanProfile::Balanced,
        };

        let widgets = view_output!();
        sender.input(FanInput::LoadProfile);
        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: Self::Input,
        sender: ComponentSender<Self>,
    ) {
        match msg {
            FanInput::LoadProfile => {
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    let msg = match fan::read_profile() {
                        Ok(profile) => FanInput::ProfileLoaded(profile),
                        Err(e) => FanInput::ReadError(e.to_string()),
                    };
                    input_sender.send(msg);
                });
            }
            FanInput::ProfileLoaded(profile) => {
                self.current_profile = profile;
            }
            FanInput::SetProfile(index) => {
                let raw = index as u8;
                if let Ok(profile) = FanProfile::from_raw(raw) {
                    self.current_profile = profile;
                    let input_sender = sender.input_sender().clone();
                    std::thread::spawn(move || {
                        input_sender.send(FanInput::ProfileWritten(
                            fan::set_profile(profile),
                        ));
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
