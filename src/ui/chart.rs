//! A live scrolling line chart (history over time), drawn with Cairo.
//! A genuinely different component from the radial gauge: filled area, grid,
//! and a moving series.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk::prelude::*;

use super::draw;
use crate::palette::Rgb;

const CAP: usize = 120;

pub struct Chart {
    pub area: gtk::DrawingArea,
    data: Rc<RefCell<VecDeque<f64>>>,
}

impl Chart {
    pub fn new(height: i32, max: f64, color: Rgb, unit: &'static str) -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_height(height);
        area.set_hexpand(true);

        let data: Rc<RefCell<VecDeque<f64>>> = Rc::new(RefCell::new(VecDeque::new()));
        let series = Rc::clone(&data);
        area.set_draw_func(move |_, cr, w, h| {
            draw(cr, w, h, &series.borrow(), max, color, unit);
        });

        Self { area, data }
    }

    /// Append the newest sample; the oldest scrolls off the left.
    pub fn push(&self, value: f64) {
        {
            let mut series = self.data.borrow_mut();
            series.push_back(value);
            while series.len() > CAP {
                series.pop_front();
            }
        }
        self.area.queue_draw();
    }
}

fn draw(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    series: &VecDeque<f64>,
    max: f64,
    color: Rgb,
    unit: &str,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    let max = max.max(1.0);

    // faint horizontal grid
    cr.set_line_width(1.0);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.06);
    for i in 1..4 {
        let y = height * f64::from(i) / 4.0;
        cr.move_to(0.0, y);
        cr.line_to(width, y);
        let _ = cr.stroke();
    }

    if series.len() < 2 {
        return;
    }

    let step = width / (CAP - 1) as f64;
    let point = |i: usize, v: f64| -> (f64, f64) {
        let x = i as f64 * step;
        let y = height - (v / max).clamp(0.0, 1.0) * (height - 4.0) - 2.0;
        (x, y)
    };

    // filled area under the line
    let (x0, _) = point(0, *series.front().unwrap_or(&0.0));
    cr.move_to(x0, height);
    for (i, &v) in series.iter().enumerate() {
        let (x, y) = point(i, v);
        cr.line_to(x, y);
    }
    let (xn, _) = point(series.len() - 1, 0.0);
    cr.line_to(xn, height);
    cr.close_path();
    draw::set_source(cr, color, 0.14);
    let _ = cr.fill();

    // the line itself, with a soft glow
    for (width_px, alpha) in [(5.0, 0.22), (2.0, 1.0)] {
        cr.set_line_width(width_px);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        draw::set_source(cr, color, alpha);
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

    // current value label, top-left
    if let Some(&last) = series.back() {
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.92);
        cr.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Bold,
        );
        cr.set_font_size(15.0);
        cr.move_to(6.0, 19.0);
        let _ = cr.show_text(&format!("{last:.0}{unit}"));
    }
}
