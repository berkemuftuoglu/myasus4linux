use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{error::BackendError, keyboard};

pub struct KeyboardPage {
    brightness: u8,
}

#[derive(Debug)]
pub enum KeyboardInput {
    LoadBrightness,
    BrightnessLoaded(u8),
    SetBrightness(u8),
    BrightnessWritten(Result<(), BackendError>),
    ReadError(String),
}

#[derive(Debug)]
pub enum KeyboardOutput {
    Error(String),
}

#[relm4::component(pub)]
impl SimpleComponent for KeyboardPage {
    type Init = ();
    type Input = KeyboardInput;
    type Output = KeyboardOutput;

    view! {
        adw::PreferencesPage {
            set_title: "Keyboard",
            set_icon_name: Some("input-keyboard-symbolic"),

            adw::PreferencesGroup {
                set_title: "Backlight",
                set_description: Some("Control the keyboard backlight brightness."),

                adw::SpinRow {
                    set_title: "Brightness",
                    set_subtitle: "Keyboard backlight level",
                    #[watch]
                    set_value: f64::from(model.brightness),
                    set_adjustment: Some(&gtk::Adjustment::new(
                        0.0,   // default
                        0.0,   // min (Off)
                        3.0,   // max (High)
                        1.0,   // step
                        1.0,   // page step
                        0.0,   // page size
                    )),
                    connect_value_notify[sender] => move |row| {
                        sender.input(KeyboardInput::SetBrightness(row.value() as u8));
                    },
                },

                adw::ActionRow {
                    set_title: "Level",
                    #[watch]
                    set_subtitle: keyboard::brightness_label(model.brightness),
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = KeyboardPage { brightness: 0 };

        let widgets = view_output!();
        sender.input(KeyboardInput::LoadBrightness);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            KeyboardInput::LoadBrightness => {
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    let msg = match keyboard::read_brightness() {
                        Ok(val) => KeyboardInput::BrightnessLoaded(val),
                        Err(e) => KeyboardInput::ReadError(e.to_string()),
                    };
                    let _ = input_sender.send(msg);
                });
            }
            KeyboardInput::BrightnessLoaded(val) => {
                self.brightness = val;
            }
            KeyboardInput::SetBrightness(val) => {
                self.brightness = val;
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    let _ = input_sender.send(KeyboardInput::BrightnessWritten(
                        keyboard::set_brightness(val),
                    ));
                });
            }
            KeyboardInput::BrightnessWritten(result) => {
                if let Err(e) = result {
                    let _ = sender.output(KeyboardOutput::Error(e.to_string()));
                }
            }
            KeyboardInput::ReadError(msg) => {
                let _ = sender.output(KeyboardOutput::Error(msg));
            }
        }
    }
}
