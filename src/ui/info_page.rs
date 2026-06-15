use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::sysfs;

/// Displays static hardware and OS information gathered at startup from
/// `/proc`, `/sys`, and standard system files.
pub struct InfoPage {
    model_name: String,
    bios_version: String,
    vendor: String,
    cpu_model: String,
    ram_total: String,
    kernel_version: String,
    storage_info: String,
}

#[derive(Debug)]
pub enum InfoInput {
    Load,
}

/// Read the CPU model name from /proc/cpuinfo.
fn read_cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| {
                    l.split_once(':')
                        .map_or_else(|| l.to_owned(), |(_, v)| v.trim().to_owned())
                })
        })
        .unwrap_or_else(|| "Unknown".to_owned())
}

/// Read total memory from /proc/meminfo and return a human-readable string.
fn read_ram_total() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|l| l.starts_with("MemTotal:"))
                .map(|l| {
                    let kb: u64 = l
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let gb = kb as f64 / 1_048_576.0;
                    format!("{gb:.1} GB")
                })
        })
        .unwrap_or_else(|| "Unknown".to_owned())
}

/// Read the kernel version from /proc/version.
fn read_kernel_version() -> String {
    std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|contents| contents.split_whitespace().nth(2).map(|s| s.to_owned()))
        .unwrap_or_else(|| "Unknown".to_owned())
}

/// Gather simple storage info by reading /proc/mounts for the root filesystem.
fn read_storage_info() -> String {
    std::fs::read_to_string("/proc/mounts")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|l| {
                    let fields: Vec<&str> = l.split_whitespace().collect();
                    fields.get(1) == Some(&"/")
                })
                .map(|l| l.split_whitespace().next().unwrap_or("Unknown").to_owned())
        })
        .unwrap_or_else(|| "Unknown".to_owned())
}

#[relm4::component(pub)]
impl SimpleComponent for InfoPage {
    type Init = ();
    type Input = InfoInput;
    type Output = ();

    view! {
        adw::PreferencesPage {
            set_title: "Info",
            set_icon_name: Some("dialog-information-symbolic"),

            adw::PreferencesGroup {
                set_title: "Device",

                adw::ActionRow {
                    set_title: "Model",
                    #[watch]
                    set_subtitle: &model.model_name,
                },

                adw::ActionRow {
                    set_title: "Vendor",
                    #[watch]
                    set_subtitle: &model.vendor,
                },

                adw::ActionRow {
                    set_title: "BIOS Version",
                    #[watch]
                    set_subtitle: &model.bios_version,
                },
            },

            adw::PreferencesGroup {
                set_title: "System",

                adw::ActionRow {
                    set_title: "CPU",
                    #[watch]
                    set_subtitle: &model.cpu_model,
                },

                adw::ActionRow {
                    set_title: "RAM",
                    #[watch]
                    set_subtitle: &model.ram_total,
                },

                adw::ActionRow {
                    set_title: "Kernel",
                    #[watch]
                    set_subtitle: &model.kernel_version,
                },

                adw::ActionRow {
                    set_title: "Root Filesystem",
                    #[watch]
                    set_subtitle: &model.storage_info,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = InfoPage {
            model_name: "Loading...".to_owned(),
            bios_version: "Loading...".to_owned(),
            vendor: "Loading...".to_owned(),
            cpu_model: "Loading...".to_owned(),
            ram_total: "Loading...".to_owned(),
            kernel_version: "Loading...".to_owned(),
            storage_info: "Loading...".to_owned(),
        };

        let widgets = view_output!();
        sender.input(InfoInput::Load);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            InfoInput::Load => {
                self.model_name = sysfs::read(crate::backend::detect::DMI_PRODUCT_NAME)
                    .unwrap_or_else(|_| "Unknown".to_owned());

                self.bios_version = sysfs::read(crate::backend::detect::DMI_BIOS_VERSION)
                    .unwrap_or_else(|_| "Unknown".to_owned());

                self.vendor = sysfs::read(crate::backend::detect::DMI_BOARD_VENDOR)
                    .unwrap_or_else(|_| "Unknown".to_owned());

                self.cpu_model = read_cpu_model();
                self.ram_total = read_ram_total();
                self.kernel_version = read_kernel_version();
                self.storage_info = read_storage_info();
            }
        }
    }
}
