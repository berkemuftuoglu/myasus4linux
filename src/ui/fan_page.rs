use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;

use adw::prelude::*;
use relm4::prelude::*;

use crate::backend::{
    cpu::CpuMonitor,
    error::BackendError,
    fan::{self, FanProfile, FanReading},
};

pub struct FanPage {
    current_profile: FanProfile,
    cpu_temp: Option<f64>,
    fans: Vec<FanReading>,
    monitor: CpuMonitor,
    gauges: Vec<CoreGauge>,
}

/// A single neon radial gauge. `target` is the real load; `shown` eases toward it
/// each frame so the arc animates smoothly instead of jumping.
struct CoreGauge {
    area: gtk::DrawingArea,
    target: Rc<Cell<f64>>,
    freq: Rc<RefCell<String>>,
}

#[derive(Debug)]
pub enum FanInput {
    Tick,
    Loaded(FanProfile, Option<f64>, Vec<FanReading>),
    SetProfile(u32),
    ProfileWritten(Result<(), BackendError>),
    ReadError(String),
}

#[derive(Debug)]
pub enum FanOutput {
    Error(String),
}

impl FanPage {
    fn fan_summary(&self) -> String {
        if self.fans.is_empty() {
            return "No fan sensor reported by this model".to_owned();
        }
        self.fans
            .iter()
            .map(|f| format!("{}: {} RPM", f.label, f.rpm))
            .collect::<Vec<_>>()
            .join("    ")
    }
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
                    set_model: Some(&gtk::StringList::new(&["Balanced", "Performance", "Quiet"])),
                    #[watch]
                    set_selected: model.current_profile as u32,
                    connect_selected_notify[sender] => move |row| {
                        sender.input(FanInput::SetProfile(row.selected()));
                    },
                },

                adw::ActionRow {
                    set_title: "CPU Temperature",
                    add_prefix = &gtk::Image { set_icon_name: Some("temperature-symbolic") },
                    #[watch]
                    set_subtitle: &model.cpu_temp
                        .map_or("Unknown".to_owned(), |t| format!("{t:.0} °C")),
                },

                adw::ActionRow {
                    set_title: "Fan Speed",
                    add_prefix = &gtk::Image { set_icon_name: Some("preferences-system-power-symbolic") },
                    #[watch]
                    set_subtitle: &model.fan_summary(),
                },
            },

            adw::PreferencesGroup {
                set_title: "CPU Cores",
                set_description: Some("Live per-thread frequency and load."),

                #[name = "cores"]
                gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_row_spacing: 12,
                    set_column_spacing: 12,
                    set_min_children_per_line: 2,
                    set_max_children_per_line: 4,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = FanPage {
            current_profile: FanProfile::Balanced,
            cpu_temp: None,
            fans: Vec::new(),
            monitor: CpuMonitor::new(),
            gauges: Vec::new(),
        };

        let widgets = view_output!();

        for core in model.monitor.sample() {
            let gauge = CoreGauge::new(core.id);
            widgets.cores.insert(&gauge.area, -1);
            model.gauges.push(gauge);
        }

        sender.input(FanInput::Tick);
        let ticker = sender.clone();
        glib::timeout_add_seconds_local(1, move || {
            ticker.input(FanInput::Tick);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            FanInput::Tick => {
                for core in self.monitor.sample() {
                    if let Some(g) = self.gauges.get(core.id) {
                        g.target.set(core.load);
                        *g.freq.borrow_mut() = format_mhz(core.mhz);
                    }
                }
                let input_sender = sender.input_sender().clone();
                std::thread::spawn(move || {
                    let msg = match fan::read_profile() {
                        Ok(profile) => {
                            FanInput::Loaded(profile, fan::read_cpu_temp(), fan::read_fans())
                        }
                        Err(e) => FanInput::ReadError(e.to_string()),
                    };
                    let _ = input_sender.send(msg);
                });
            }
            FanInput::Loaded(profile, temp, fans) => {
                self.current_profile = profile;
                self.cpu_temp = temp;
                self.fans = fans;
            }
            FanInput::SetProfile(index) => {
                if let Ok(profile) = FanProfile::from_raw(index as u8) {
                    self.current_profile = profile;
                    let input_sender = sender.input_sender().clone();
                    std::thread::spawn(move || {
                        let _ =
                            input_sender.send(FanInput::ProfileWritten(fan::set_profile(profile)));
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

impl CoreGauge {
    fn new(id: usize) -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_width(132);
        area.set_content_height(132);

        let target = Rc::new(Cell::new(0.0));
        let shown = Rc::new(Cell::new(0.0));
        let freq = Rc::new(RefCell::new("—".to_owned()));

        let (d_shown, d_freq) = (Rc::clone(&shown), Rc::clone(&freq));
        area.set_draw_func(move |_, cr, w, h| {
            draw_gauge(cr, w, h, id, d_shown.get(), &d_freq.borrow());
        });

        // Ease the shown value toward the target every frame for a smooth sweep.
        let (a_target, a_shown) = (Rc::clone(&target), Rc::clone(&shown));
        area.add_tick_callback(move |area, _clock| {
            let (t, s) = (a_target.get(), a_shown.get());
            if (t - s).abs() > 0.1 {
                a_shown.set(s + (t - s) * 0.18);
                area.queue_draw();
            }
            glib::ControlFlow::Continue
        });

        Self { area, target, freq }
    }
}

fn format_mhz(mhz: u32) -> String {
    if mhz == 0 {
        "—".to_owned()
    } else {
        format!("{:.1}", f64::from(mhz) / 1000.0)
    }
}

/// Neon green (idle) through amber to red (pegged).
fn load_color(load: f64) -> (f64, f64, f64) {
    let t = (load / 100.0).clamp(0.0, 1.0);
    if t < 0.5 {
        lerp((0.27, 0.84, 0.17), (1.0, 0.72, 0.0), t / 0.5)
    } else {
        lerp((1.0, 0.72, 0.0), (1.0, 0.25, 0.25), (t - 0.5) / 0.5)
    }
}

fn lerp(a: (f64, f64, f64), b: (f64, f64, f64), k: f64) -> (f64, f64, f64) {
    (
        a.0 + (b.0 - a.0) * k,
        a.1 + (b.1 - a.1) * k,
        a.2 + (b.2 - a.2) * k,
    )
}

fn draw_gauge(cr: &gtk::cairo::Context, width: i32, height: i32, id: usize, load: f64, freq: &str) {
    let width = f64::from(width);
    let height = f64::from(height);
    let cx = width / 2.0;
    let cy = height / 2.0;
    let radius = width.min(height) / 2.0 - 14.0;
    let start = 0.75 * PI;
    let span = 1.5 * PI;
    let frac = (load / 100.0).clamp(0.0, 1.0);
    let (red, green, blue) = load_color(load);

    cr.set_line_cap(gtk::cairo::LineCap::Round);

    // background track
    cr.set_line_width(10.0);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
    cr.arc(cx, cy, radius, start, start + span);
    let _ = cr.stroke();

    if frac > 0.0 {
        // outer glow
        cr.set_line_width(20.0);
        cr.set_source_rgba(red, green, blue, 0.16);
        cr.arc(cx, cy, radius, start, start + span * frac);
        let _ = cr.stroke();
        // core arc
        cr.set_line_width(10.0);
        cr.set_source_rgba(red, green, blue, 1.0);
        cr.arc(cx, cy, radius, start, start + span * frac);
        let _ = cr.stroke();
    }

    // centre: frequency in GHz
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.96);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Bold,
    );
    cr.set_font_size(26.0);
    center_text(cr, freq, cx, cy + 2.0);

    cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
    cr.set_font_size(10.0);
    center_text(cr, "GHz", cx, cy + 18.0);

    // bottom label: core + load%
    cr.set_source_rgba(red, green, blue, 0.9);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Normal,
    );
    cr.set_font_size(11.0);
    center_text(
        cr,
        &format!("Core {id}  ·  {load:.0}%"),
        cx,
        cy + radius - 2.0,
    );
}

fn center_text(cr: &gtk::cairo::Context, text: &str, cx: f64, y: f64) {
    if let Ok(ext) = cr.text_extents(text) {
        cr.move_to(cx - ext.width() / 2.0 - ext.x_bearing(), y);
        let _ = cr.show_text(text);
    }
}
