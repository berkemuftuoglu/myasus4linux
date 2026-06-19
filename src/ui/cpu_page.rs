use adw::prelude::*;
use relm4::prelude::*;

use super::chart::Chart;
use super::gauge::{Accent, Gauge};
use super::ledbar::LedBar;
use super::panel::Panel;
use super::stat::Stat;
use crate::backend::{cpu::CpuMonitor, fan};
use crate::palette::{self, Rgb};

pub struct CpuPage {
    monitor: CpuMonitor,
    load_g: Gauge,
    temp_g: Gauge,
    freq_s: Stat,
    core_panel: Panel,
    core_leds: Vec<LedBar>,
    temp_chart: Chart,
    load_chart: Chart,
}

#[derive(Debug)]
pub enum CpuInput {
    Tick,
}

#[relm4::component(pub)]
impl SimpleComponent for CpuPage {
    type Init = ();
    type Input = CpuInput;
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
                    set_label: "CPU",
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

                #[name = "trends"]
                gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_column_spacing: 14,
                    set_row_spacing: 14,
                    set_min_children_per_line: 1,
                    set_max_children_per_line: 2,
                },

                #[name = "cores_slot"]
                gtk::Box {},
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = CpuPage {
            monitor: CpuMonitor::new(),
            load_g: Gauge::new(168, Accent::ByValue),
            temp_g: Gauge::new(168, Accent::ByValue),
            freq_s: Stat::with_spark("Average Frequency", palette::GOOD),
            core_panel: Panel::new("Per-core Load and Frequency"),
            core_leds: Vec::new(),
            temp_chart: Chart::new(140, 100.0, Rgb::new(1.0, 0.6, 0.2), "°"),
            load_chart: Chart::new(140, 100.0, palette::GOOD, "%"),
        };

        let widgets = view_output!();

        widgets.heroes.insert(
            &Panel::metric("Total Load", &model.load_g.area, 240, true),
            -1,
        );
        widgets.heroes.insert(
            &Panel::metric("Package Temperature", &model.temp_g.area, 240, true),
            -1,
        );
        model.freq_s.root.set_size_request(200, -1);
        widgets.heroes.insert(&model.freq_s.root, -1);

        widgets.trends.insert(
            &Panel::metric("Total Load", &model.load_chart.area, 340, false),
            -1,
        );
        widgets.trends.insert(
            &Panel::metric("Temperature", &model.temp_chart.area, 340, false),
            -1,
        );

        model
            .core_panel
            .body
            .set_orientation(gtk::Orientation::Vertical);
        model.core_panel.body.set_spacing(8);
        for core in model.monitor.sample() {
            let led = LedBar::new(&format!("Core {}", core.id));
            model.core_panel.body.append(&led.root);
            model.core_leds.push(led);
        }
        widgets.cores_slot.append(&model.core_panel.root);

        sender.input(CpuInput::Tick);
        let ticker = sender.clone();
        glib::timeout_add_seconds_local(1, move || {
            ticker.input(CpuInput::Tick);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            CpuInput::Tick => {
                let cores = self.monitor.sample();
                let n = cores.len().max(1) as f64;
                let load = cores.iter().map(|c| c.load).sum::<f64>() / n;
                let freq = f64::from(cores.iter().map(|c| c.mhz).sum::<u32>()) / n / 1000.0;

                self.load_g
                    .set(load / 100.0, &format!("{load:.0}%"), "all cores");
                self.load_chart.push(load);
                self.freq_s.set(&format!("{freq:.1}"), "GHz, all cores");
                self.freq_s.push(freq);

                for (core, led) in cores.iter().zip(&self.core_leds) {
                    led.set(
                        core.load / 100.0,
                        &format!("{:.1}GHz", f64::from(core.mhz) / 1000.0),
                    );
                }
                self.core_panel.set_corner(&format!("avg {load:.0}%"));

                if let Some(temp) = fan::read_cpu_temp() {
                    self.temp_g
                        .set(temp / 100.0, &format!("{temp:.0}°"), "package");
                    self.temp_chart.push(temp);
                }
            }
        }
    }
}
