use adw::prelude::*;
use relm4::prelude::*;

use crate::ui::widgets::meter::Meter;
use crate::ui::widgets::panel::Panel;
use crate::ui::widgets::stat::Stat;
use crate::ui::widgets::table::Table;
use crate::backend::{detect, sysfs};

/// Hardware and OS facts. Static identity is shown as a spec table; memory and
/// uptime update live in panels above it.
pub struct InfoPage {
    mem_meter: Meter,
    uptime_s: Stat,
}

#[derive(Debug)]
pub enum InfoInput {
    Tick,
    Loaded(Option<(f64, f64)>, String),
}

#[relm4::component(pub)]
impl SimpleComponent for InfoPage {
    type Init = ();
    type Input = InfoInput;
    type Output = ();

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
                    set_label: "System",
                    add_css_class: "title-1",
                },

                #[name = "live"]
                gtk::Box { set_homogeneous: true, set_spacing: 14 },

                #[name = "table_slot"]
                gtk::Box {},
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = InfoPage {
            mem_meter: Meter::new("Memory"),
            uptime_s: Stat::new("Uptime"),
        };

        let widgets = view_output!();

        let mem_panel = Panel::new("Memory Usage");
        mem_panel.body.append(&model.mem_meter.root);
        mem_panel.root.set_hexpand(true);
        widgets.live.append(&mem_panel.root);
        widgets.live.append(&model.uptime_s.root);

        let table = Table::new(
            "Hardware Configuration",
            &[
                ("Model", dmi(detect::DMI_PRODUCT_NAME)),
                ("Product Family", dmi(detect::DMI_PRODUCT_FAMILY)),
                ("Vendor", dmi(detect::DMI_BOARD_VENDOR)),
                ("Board", dmi(detect::DMI_BOARD_NAME)),
                ("BIOS Version", dmi(detect::DMI_BIOS_VERSION)),
                ("Processor", read_cpu_model()),
                ("Memory", read_ram_total()),
                ("Kernel", read_kernel_version()),
                ("Root Filesystem", read_storage_info()),
            ],
        );
        widgets.table_slot.append(&table.root);

        sender.input(InfoInput::Tick);
        let ticker = sender.clone();
        glib::timeout_add_seconds_local(crate::ui::POLL_SECS, move || {
            ticker.input(InfoInput::Tick);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            InfoInput::Tick => {
                crate::ui::offload(sender.input_sender(), || {
                    InfoInput::Loaded(read_mem_usage(), read_uptime())
                });
            }
            InfoInput::Loaded(mem, uptime) => {
                if let Some((used, total)) = mem {
                    let frac = if total > 0.0 { used / total } else { 0.0 };
                    self.mem_meter.set(frac, &format!("{used:.1}/{total:.0}G"));
                }
                self.uptime_s.set(&uptime, "since boot");
            }
        }
    }
}

fn dmi(path: &str) -> String {
    sysfs::read(path).unwrap_or_else(|_| "Unknown".to_owned())
}

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

fn read_meminfo_kb(key: &str) -> Option<u64> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    crate::format::meminfo_kb(&contents, key)
}

fn read_ram_total() -> String {
    read_meminfo_kb("MemTotal:").map_or_else(
        || "Unknown".to_owned(),
        |kb| format!("{:.1} GB", kb as f64 / 1_048_576.0),
    )
}

/// Used and total memory in GiB, derived from `MemTotal` and `MemAvailable`.
fn read_mem_usage() -> Option<(f64, f64)> {
    let total = read_meminfo_kb("MemTotal:")?;
    let available = read_meminfo_kb("MemAvailable:")?;
    let gib = |kb: u64| kb as f64 / 1_048_576.0;
    Some((gib(total.saturating_sub(available)), gib(total)))
}

fn read_uptime() -> String {
    let secs = crate::num::round_u32(
        std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
            .unwrap_or(0.0),
    );
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}

fn read_kernel_version() -> String {
    std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|contents| contents.split_whitespace().nth(2).map(ToOwned::to_owned))
        .unwrap_or_else(|| "Unknown".to_owned())
}

fn read_storage_info() -> String {
    std::fs::read_to_string("/proc/mounts")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|l| l.split_whitespace().nth(1) == Some("/"))
                .map(|l| l.split_whitespace().next().unwrap_or("Unknown").to_owned())
        })
        .unwrap_or_else(|| "Unknown".to_owned())
}
