use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::detect::{self, HardwareFeatures};
use super::battery_page::BatteryPage;
use super::fan_page::FanPage;
use super::info_page::InfoPage;
use super::keyboard_page::KeyboardPage;

/// Top-level application component.
///
/// Holds child page controllers and the detected hardware features.
pub struct App {
    features: HardwareFeatures,
    battery_page: Option<Controller<BatteryPage>>,
    fan_page: Option<Controller<FanPage>>,
    keyboard_page: Option<Controller<KeyboardPage>>,
    info_page: Controller<InfoPage>,
}

#[derive(Debug)]
pub enum AppInput {
    ShowToast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = AppInput;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("MyASUS for Linux"),
            set_default_width: 800,
            set_default_height: 600,

            adw::ToolbarView {
                // Header bar with view switcher
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::ViewSwitcher {
                        set_stack: Some(&stack),
                        set_policy: adw::ViewSwitcherPolicy::Wide,
                    },
                },

                // Toast overlay wrapping the view stack
                #[wrap(Some)]
                set_content = &adw::ToastOverlay {
                    #[local_ref]
                    stack -> adw::ViewStack {},
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let features = detect::detect_features();

        // Build child page controllers only for available features
        let battery_page = if features.battery {
            Some(
                BatteryPage::builder()
                    .launch(())
                    .forward(sender.input_sender(), |msg| match msg {
                        super::battery_page::BatteryOutput::Error(e) => {
                            AppInput::ShowToast(e)
                        }
                    }),
            )
        } else {
            None
        };

        let fan_page = if features.fan_profile {
            Some(
                FanPage::builder()
                    .launch(())
                    .forward(sender.input_sender(), |msg| match msg {
                        super::fan_page::FanOutput::Error(e) => {
                            AppInput::ShowToast(e)
                        }
                    }),
            )
        } else {
            None
        };

        let keyboard_page = if features.keyboard_backlight {
            Some(
                KeyboardPage::builder()
                    .launch(())
                    .forward(sender.input_sender(), |msg| match msg {
                        super::keyboard_page::KeyboardOutput::Error(e) => {
                            AppInput::ShowToast(e)
                        }
                    }),
            )
        } else {
            None
        };

        let info_page = InfoPage::builder()
            .launch(())
            .detach();

        // Build the ViewStack and add pages
        let stack = adw::ViewStack::new();

        if let Some(ref controller) = battery_page {
            let page = stack.add_titled(
                controller.widget(),
                Some("battery"),
                "Battery",
            );
            page.set_icon_name(Some("battery-symbolic"));
        }

        if let Some(ref controller) = fan_page {
            let page = stack.add_titled(
                controller.widget(),
                Some("fan"),
                "Fan",
            );
            page.set_icon_name(Some("preferences-system-power-symbolic"));
        }

        if let Some(ref controller) = keyboard_page {
            let page = stack.add_titled(
                controller.widget(),
                Some("keyboard"),
                "Keyboard",
            );
            page.set_icon_name(Some("input-keyboard-symbolic"));
        }

        let info_stack_page = stack.add_titled(
            info_page.widget(),
            Some("info"),
            "Info",
        );
        info_stack_page.set_icon_name(Some("dialog-information-symbolic"));

        // If no hardware-specific pages are available, the user still sees Info.

        let model = App {
            features,
            battery_page,
            fan_page,
            keyboard_page,
            info_page,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppInput::ShowToast(text) => {
                tracing::warn!("{text}");
                // In a full implementation the toast overlay reference would be
                // stored so we can call add_toast() here.
            }
        }
    }
}
