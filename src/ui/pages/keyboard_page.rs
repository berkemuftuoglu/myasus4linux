use adw::prelude::*;
use relm4::prelude::*;

use crate::ui::widgets::ledbar::LedBar;
use crate::backend::{brightness, error::BackendError, keyboard};
use crate::ui::palette;

pub struct KeyboardPage {
    brightness: u8,
    // True while our own backlight write is in flight, so a poll doesn't stomp
    // the optimistic value mid-write.
    kbd_pending: bool,
    screen: crate::ui::commit::DebouncedCommit<u8>,
    screen_available: bool,
    level: LedBar,
}

#[derive(Debug)]
pub enum KeyboardInput {
    LoadBrightness,
    BrightnessLoaded(u8),
    SetBrightness(u8),
    BrightnessWritten {
        result: Result<(), BackendError>,
        prev: u8,
    },
    LoadScreen,
    ScreenLoaded(Option<u8>),
    ScreenMoved(u8),
    CommitScreen(u32),
    ScreenWritten(Result<(), BackendError>),
    ReadError(String),
}

#[relm4::component(pub)]
impl SimpleComponent for KeyboardPage {
    type Init = ();
    type Input = KeyboardInput;
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
                    set_label: "Lighting",
                    add_css_class: "title-1",
                },

                gtk::Box {
                    add_css_class: "panel",
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Box {
                        add_css_class: "panel-header",
                        gtk::Label {
                            set_label: "Keyboard Backlight",
                            add_css_class: "panel-title",
                            set_halign: gtk::Align::Start,
                            set_hexpand: true,
                        },
                        gtk::Label {
                            add_css_class: "panel-corner",
                            #[watch]
                            set_label: keyboard::brightness_label(model.brightness),
                        },
                    },

                    gtk::Box {
                        add_css_class: "panel-body",
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,

                        gtk::Box {
                            set_homogeneous: true,
                            add_css_class: "linked",

                            #[name = "kbd_off"]
                            gtk::ToggleButton {
                                set_label: "Off",
                                #[watch]
                                set_active: model.brightness == 0,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(KeyboardInput::SetBrightness(0));
                                },
                            },
                            #[name = "kbd_low"]
                            gtk::ToggleButton {
                                set_label: "Low",
                                #[watch]
                                set_active: model.brightness == 1,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(KeyboardInput::SetBrightness(1));
                                },
                            },
                            #[name = "kbd_med"]
                            gtk::ToggleButton {
                                set_label: "Medium",
                                #[watch]
                                set_active: model.brightness == 2,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(KeyboardInput::SetBrightness(2));
                                },
                            },
                            #[name = "kbd_high"]
                            gtk::ToggleButton {
                                set_label: "High",
                                #[watch]
                                set_active: model.brightness == 3,
                                connect_clicked[sender] => move |b| if b.is_active() {
                                    sender.input(KeyboardInput::SetBrightness(3));
                                },
                            },
                        },

                        #[name = "level_slot"]
                        gtk::Box {},
                    },
                },

                gtk::Box {
                    add_css_class: "panel",
                    set_orientation: gtk::Orientation::Vertical,
                    set_visible: model.screen_available,

                    gtk::Box {
                        add_css_class: "panel-header",
                        gtk::Label {
                            set_label: "Display Brightness",
                            add_css_class: "panel-title",
                            set_halign: gtk::Align::Start,
                            set_hexpand: true,
                        },
                        gtk::Label {
                            add_css_class: "panel-corner",
                            #[watch]
                            set_label: &format!("{}%", model.screen.value()),
                        },
                    },

                    gtk::Box {
                        add_css_class: "panel-body",

                        #[name = "screen_scale"]
                        gtk::Scale {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_hexpand: true,
                            set_draw_value: false,
                            set_round_digits: 0,
                            set_adjustment: &gtk::Adjustment::new(50.0, 5.0, 100.0, 5.0, 10.0, 0.0),
                            #[watch]
                            #[block_signal(screen_changed)]
                            set_value: f64::from(model.screen.value()),
                            connect_value_changed[sender] => move |s| {
                                sender.input(KeyboardInput::ScreenMoved(crate::num::round_u8_in(s.value(), 5, 100)));
                            } @screen_changed,
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
        let screen = brightness::read_percent().unwrap_or(50);
        let model = KeyboardPage {
            brightness: 0,
            kbd_pending: false,
            screen: crate::ui::commit::DebouncedCommit::new(screen),
            screen_available: brightness::available(),
            level: LedBar::accent("Level", palette::GOOD),
        };

        let widgets = view_output!();

        widgets.kbd_low.set_group(Some(&widgets.kbd_off));
        widgets.kbd_med.set_group(Some(&widgets.kbd_off));
        widgets.kbd_high.set_group(Some(&widgets.kbd_off));
        widgets.level_slot.append(&model.level.root);

        for mark in [25.0, 50.0, 75.0, 100.0] {
            widgets
                .screen_scale
                .add_mark(mark, gtk::PositionType::Bottom, None);
        }

        sender.input(KeyboardInput::LoadBrightness);
        sender.input(KeyboardInput::LoadScreen);
        // Poll so external changes (the Fn backlight key, screen auto-dim, other
        // tools) are reflected -- those write sysfs directly and emit no D-Bus
        // signal, so a poll is the only way to catch them.
        crate::ui::poll(&root, sender.input_sender(), || KeyboardInput::LoadBrightness);
        crate::ui::poll(&root, sender.input_sender(), || KeyboardInput::LoadScreen);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            KeyboardInput::LoadBrightness => {
                crate::ui::offload(sender.input_sender(), || {
                    match keyboard::read_brightness() {
                        Ok(val) => KeyboardInput::BrightnessLoaded(val),
                        Err(e) => KeyboardInput::ReadError(e.to_string()),
                    }
                });
            }
            KeyboardInput::BrightnessLoaded(val) => {
                // Don't stomp the optimistic value while our own write is pending.
                if self.kbd_pending {
                    return;
                }
                self.brightness = val;
                self.level
                    .set(f64::from(val) / 3.0, keyboard::brightness_label(val));
            }
            KeyboardInput::SetBrightness(val) => {
                if val == self.brightness {
                    return;
                }
                let prev = self.brightness;
                self.brightness = val;
                self.kbd_pending = true;
                self.level
                    .set(f64::from(val) / 3.0, keyboard::brightness_label(val));
                crate::ui::offload(sender.input_sender(), move || {
                    KeyboardInput::BrightnessWritten {
                        result: keyboard::set_brightness(val),
                        prev,
                    }
                });
            }
            KeyboardInput::BrightnessWritten { result, prev } => {
                self.kbd_pending = false;
                if let Err(e) = result {
                    // The write failed, so the hardware never changed: undo the
                    // optimistic UI so it doesn't lie about the active level.
                    self.brightness = prev;
                    self.level
                        .set(f64::from(prev) / 3.0, keyboard::brightness_label(prev));
                    let _ = sender.output(crate::ui::PageMsg::Error(e.to_string()));
                } else {
                    let _ = sender.output(crate::ui::PageMsg::Notice(format!(
                        "Keyboard backlight: {}",
                        keyboard::brightness_label(self.brightness)
                    )));
                }
            }
            KeyboardInput::LoadScreen => {
                crate::ui::offload(sender.input_sender(), || {
                    KeyboardInput::ScreenLoaded(brightness::read_percent())
                });
            }
            KeyboardInput::ScreenLoaded(value) => {
                // poll() ignores this while our own write is in flight.
                if let Some(v) = value {
                    self.screen.poll(v);
                }
            }
            KeyboardInput::ScreenMoved(val) => {
                // Optimistic display now; defer the write until the drag settles
                // so a drag is one write, not one per step.
                if let Some(seq) = self.screen.slide(val) {
                    let s = sender.clone();
                    glib::timeout_add_local(
                        std::time::Duration::from_millis(crate::ui::COMMIT_DEBOUNCE_MS),
                        move || {
                            s.input(KeyboardInput::CommitScreen(seq));
                            glib::ControlFlow::Break
                        },
                    );
                }
            }
            KeyboardInput::CommitScreen(seq) => {
                if let Some(val) = self.screen.commit(seq) {
                    crate::ui::offload(sender.input_sender(), move || {
                        KeyboardInput::ScreenWritten(brightness::set_percent(val))
                    });
                }
            }
            KeyboardInput::ScreenWritten(result) => {
                self.screen.written(result.is_ok());
                if let Err(e) = result {
                    let _ = sender.output(crate::ui::PageMsg::Error(e.to_string()));
                }
            }
            KeyboardInput::ReadError(msg) => {
                let _ = sender.output(crate::ui::PageMsg::Error(msg));
            }
        }
    }
}
