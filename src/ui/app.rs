use adw::prelude::*;
use relm4::prelude::*;

use super::pages::battery_page::BatteryPage;
use super::pages::cpu_page::CpuPage;
use super::pages::fan_page::FanPage;
use super::pages::info_page::InfoPage;
use super::pages::keyboard_page::KeyboardPage;
use super::pages::overview::Overview;
use crate::backend::detect;

/// Top-level application component.
///
/// The page controllers are held only to keep their components alive; dropping
/// them would tear down the pages, so they are stored but never read directly.
pub struct App {
    toaster: adw::ToastOverlay,
    _overview: Controller<Overview>,
    _battery_page: Option<Controller<BatteryPage>>,
    _cpu_page: Controller<CpuPage>,
    _fan_page: Option<Controller<FanPage>>,
    _keyboard_page: Option<Controller<KeyboardPage>>,
    _info_page: Controller<InfoPage>,
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
            set_default_width: 1040,
            set_default_height: 720,

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &gtk::Label {
                        set_label: "MyASUS",
                        add_css_class: "title-4",
                    },
                },

                #[wrap(Some)]
                #[local_ref]
                set_content = &toaster -> adw::ToastOverlay {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_size_request: (212, -1),

                            #[local_ref]
                            nav -> gtk::ListBox {
                                set_vexpand: true,
                                add_css_class: "navigation-sidebar",
                            },
                        },

                        gtk::Separator { set_orientation: gtk::Orientation::Vertical },

                        #[local_ref]
                        stack -> gtk::Stack {
                            set_hexpand: true,
                            set_vexpand: true,
                            set_transition_type: gtk::StackTransitionType::Crossfade,
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
        apply_theme();

        let features = detect::detect_features();
        let toaster = adw::ToastOverlay::new();

        let overview =
            Overview::builder()
                .launch(features.clone())
                .forward(sender.input_sender(), |msg| match msg {
                    super::pages::overview::OverviewOutput::Error(e) => AppInput::ShowToast(e),
                });

        let battery_page = features.battery.then(|| {
            BatteryPage::builder()
                .launch(features.charge_limit)
                .forward(sender.input_sender(), |msg| match msg {
                    super::pages::battery_page::BatteryOutput::Error(e) => AppInput::ShowToast(e),
                })
        });

        let fan_page = features.fan_profile.then(|| {
            FanPage::builder()
                .launch(())
                .forward(sender.input_sender(), |msg| match msg {
                    super::pages::fan_page::FanOutput::Error(e) => AppInput::ShowToast(e),
                })
        });

        let keyboard_page = features.keyboard_backlight.then(|| {
            KeyboardPage::builder()
                .launch(())
                .forward(sender.input_sender(), |msg| match msg {
                    super::pages::keyboard_page::KeyboardOutput::Error(e) => AppInput::ShowToast(e),
                })
        });

        let cpu_page = CpuPage::builder().launch(()).detach();
        let info_page = InfoPage::builder().launch(()).detach();

        // Source of truth for the nav rows and stack pages, in order.
        let mut entries: Vec<(&str, &str, &str, gtk::Widget)> = vec![(
            "overview",
            "Overview",
            "view-grid-symbolic",
            overview.widget().clone().upcast(),
        )];
        if let Some(c) = &battery_page {
            entries.push((
                "battery",
                "Battery",
                "battery-symbolic",
                c.widget().clone().upcast(),
            ));
        }
        entries.push((
            "cpu",
            "CPU",
            "speedometer-symbolic",
            cpu_page.widget().clone().upcast(),
        ));
        if let Some(c) = &fan_page {
            entries.push((
                "cooling",
                "Cooling",
                "preferences-system-power-symbolic",
                c.widget().clone().upcast(),
            ));
        }
        if let Some(c) = &keyboard_page {
            entries.push((
                "lighting",
                "Lighting",
                "display-brightness-symbolic",
                c.widget().clone().upcast(),
            ));
        }
        entries.push((
            "system",
            "System",
            "dialog-information-symbolic",
            info_page.widget().clone().upcast(),
        ));

        let (stack, nav) = build_navigation(&entries);

        let model = App {
            toaster: toaster.clone(),
            _overview: overview,
            _battery_page: battery_page,
            _cpu_page: cpu_page,
            _fan_page: fan_page,
            _keyboard_page: keyboard_page,
            _info_page: info_page,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppInput::ShowToast(text) => {
                tracing::warn!("{text}");
                self.toaster.add_toast(adw::Toast::new(&text));
            }
        }
    }
}

/// Build the sidebar list and the page stack from the ordered nav entries, and
/// wire row activation to switch the visible page.
fn build_navigation(entries: &[(&str, &str, &str, gtk::Widget)]) -> (gtk::Stack, gtk::ListBox) {
    let stack = gtk::Stack::new();
    let nav = gtk::ListBox::new();
    for (name, title, icon, widget) in entries {
        stack.add_named(widget, Some(name));
        nav.append(&nav_row(title, icon));
    }

    let names: Vec<String> = entries.iter().map(|(n, ..)| (*n).to_owned()).collect();
    let stack_for_nav = stack.clone();
    nav.connect_row_activated(move |_, row| {
        let index = usize::try_from(row.index()).unwrap_or(0);
        if let Some(name) = names.get(index) {
            stack_for_nav.set_visible_child_name(name);
        }
    });
    if let Some(first) = nav.row_at_index(0) {
        nav.select_row(Some(&first));
    }
    (stack, nav)
}

fn nav_row(title: &str, icon: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let layout = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    layout.set_margin_top(10);
    layout.set_margin_bottom(10);
    layout.set_margin_start(12);
    layout.set_margin_end(12);
    layout.append(&gtk::Image::from_icon_name(icon));

    let label = gtk::Label::new(Some(title));
    label.set_halign(gtk::Align::Start);
    layout.append(&label);

    row.set_child(Some(&layout));
    row
}

/// Force a dark base and load the premium stylesheet for the whole app.
fn apply_theme() {
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
