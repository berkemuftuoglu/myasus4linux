//! A stat panel: panel header label, one big tabular number, a small caption,
//! and an optional sparkline of the recent history. For instant scalar values
//! (power, time, frequency, voltage) on the dashboard.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk::prelude::*;

use super::draw;
use super::panel::Panel;
use crate::palette::Rgb;

const SPARK_CAP: usize = 60;

pub struct Stat {
    pub root: gtk::Box,
    value: gtk::Label,
    sub: gtk::Label,
    spark: Option<Spark>,
}

impl Stat {
    pub fn new(label: &str) -> Self {
        Self::build(label, None)
    }

    /// A stat panel with an autoscaling sparkline of the recent window beneath
    /// the number. Autoscaling matters for slow scalars like voltage: a fixed
    /// scale would draw them as a dead flat line.
    pub fn with_spark(label: &str, color: Rgb) -> Self {
        Self::build(label, Some(color))
    }

    fn build(label: &str, spark: Option<Rgb>) -> Self {
        let panel = Panel::new(label);
        panel.body.set_spacing(2);

        let value = gtk::Label::new(Some("—"));
        value.add_css_class("stat-value");
        value.set_halign(gtk::Align::Start);

        let sub = gtk::Label::new(Some(""));
        sub.add_css_class("dim-label");
        sub.set_halign(gtk::Align::Start);

        panel.body.append(&value);
        panel.body.append(&sub);

        let spark = spark.map(|color| {
            let s = Spark::new(color);
            s.area.set_margin_top(6);
            panel.body.append(&s.area);
            s
        });

        Self {
            root: panel.root,
            value,
            sub,
            spark,
        }
    }

    pub fn set(&self, value: &str, sub: &str) {
        self.value.set_text(value);
        self.sub.set_text(sub);
    }

    /// Append a sample to the sparkline (no-op if this stat has none).
    pub fn push(&self, sample: f64) {
        if let Some(spark) = &self.spark {
            spark.push(sample);
        }
    }
}

struct Spark {
    area: gtk::DrawingArea,
    data: Rc<RefCell<VecDeque<f64>>>,
}

impl Spark {
    fn new(color: Rgb) -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_height(30);
        area.set_hexpand(true);

        let data: Rc<RefCell<VecDeque<f64>>> = Rc::new(RefCell::new(VecDeque::new()));
        let series = Rc::clone(&data);
        area.set_draw_func(move |_, cr, w, h| draw_spark(cr, w, h, &series.borrow(), color));

        Self { area, data }
    }

    fn push(&self, sample: f64) {
        {
            let mut series = self.data.borrow_mut();
            series.push_back(sample);
            while series.len() > SPARK_CAP {
                series.pop_front();
            }
        }
        self.area.queue_draw();
    }
}

fn draw_spark(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    series: &VecDeque<f64>,
    color: Rgb,
) {
    if series.len() < 2 {
        return;
    }
    let width = f64::from(width);
    let height = f64::from(height);
    // autoscale to the window's own min/max so small wobbles are visible
    let lo = series.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = series.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = (hi - lo).max(1e-6);
    let step = width / (SPARK_CAP - 1) as f64;
    let point = |i: usize, v: f64| -> (f64, f64) {
        let x = i as f64 * step;
        let y = height - ((v - lo) / range) * (height - 4.0) - 2.0;
        (x, y)
    };

    cr.move_to(0.0, height);
    for (i, &v) in series.iter().enumerate() {
        let (x, y) = point(i, v);
        cr.line_to(x, y);
    }
    cr.line_to((series.len() - 1) as f64 * step, height);
    cr.close_path();
    draw::set_source(cr, color, 0.16);
    let _ = cr.fill();

    cr.set_line_width(1.6);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    draw::set_source(cr, color, 0.95);
    for (i, &v) in series.iter().enumerate() {
        let (x, y) = point(i, v);
        if i == 0 {
            cr.move_to(x, y);
        } else {
            cr.line_to(x, y);
        }
    }
    let _ = cr.stroke();
}
