use adw::prelude::*;
use relm4::prelude::*;

use crate::ui::widgets::chart::Chart;
use crate::ui::widgets::gauge::{Accent, Gauge};
use crate::ui::widgets::ledbar::LedBar;
use crate::ui::widgets::panel::Panel;
use crate::ui::widgets::stat::Stat;
use crate::backend::{
    cpu::{CoreStat, CpuMonitor},
    fan,
};
use crate::ui::palette::{self, Rgb};

pub struct CpuPage {
    // The monitor lives behind an Option so it can be handed to the worker
    // thread for a sample and handed back on return, keeping its cross-sample
    // delta state without ever reading sysfs on the GTK main thread.
    monitor: Option<CpuMonitor>,
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
    Sampled(Box<CpuSample>),
}

/// Plain data carried back from the worker thread: no GTK, no borrowed state.
#[derive(Debug)]
pub struct CpuSample {
    monitor: CpuMonitor,
    cores: Vec<CoreStat>,
    temp: Option<f64>,
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
        let mut monitor = CpuMonitor::new();
        let cores = monitor.sample();

        let mut model = CpuPage {
            monitor: Some(monitor),
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

        crate::ui::builders::cores::build(&model.core_panel, &mut model.core_leds, &cores);
        widgets.cores_slot.append(&model.core_panel.root);

        sender.input(CpuInput::Tick);
        let ticker = sender.clone();
        glib::timeout_add_seconds_local(crate::ui::POLL_SECS, move || {
            ticker.input(CpuInput::Tick);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            CpuInput::Tick => {
                // Skip if the previous sample is still running so reads can't pile up.
                if let Some(mut monitor) = self.monitor.take() {
                    crate::ui::offload(sender.input_sender(), move || {
                        let cores = monitor.sample();
                        let temp = fan::read_cpu_temp();
                        CpuInput::Sampled(Box::new(CpuSample {
                            monitor,
                            cores,
                            temp,
                        }))
                    });
                }
            }
            CpuInput::Sampled(sample) => {
                let CpuSample {
                    monitor,
                    cores,
                    temp,
                } = *sample;
                self.monitor = Some(monitor);

                let n = cores.len().max(1) as f64;
                let load = cores.iter().map(|c| c.load).sum::<f64>() / n;
                let freq = f64::from(cores.iter().map(|c| c.mhz).sum::<u32>()) / n / 1000.0;

                self.load_g
                    .set(load / 100.0, &format!("{load:.0}%"), "all cores");
                self.load_chart.push(load);
                self.freq_s.set(&format!("{freq:.1}"), "GHz, all cores");
                self.freq_s.push(freq);

                crate::ui::builders::cores::update(&self.core_leds, &cores);
                self.core_panel.set_corner(&format!("avg {load:.0}%"));

                if let Some(temp) = temp {
                    self.temp_g
                        .set(temp / 100.0, &format!("{temp:.0}°"), "package");
                    self.temp_chart.push(temp);
                }
            }
        }
    }
}
