//! A bullet-bar meter: label · range-banded bar · tabular value. A bar packs a
//! dense, scannable matrix of comparable readings (sensors, per-core load) into
//! far less width than one gauge each would take.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use gtk::prelude::*;

use super::draw;
use crate::ui::palette;

pub struct Meter {
    pub root: gtk::Box,
    bar: gtk::DrawingArea,
    value_label: gtk::Label,
    frac: Rc<Cell<f64>>,
}

impl Meter {
    pub fn new(name: &str) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 12);

        let name_label = gtk::Label::new(Some(name));
        name_label.set_halign(gtk::Align::Start);
        name_label.set_width_chars(14);
        name_label.set_xalign(0.0);
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.add_css_class("dim-label");

        let bar = gtk::DrawingArea::new();
        bar.set_hexpand(true);
        bar.set_content_height(18);
        bar.set_valign(gtk::Align::Center);

        let value_label = gtk::Label::new(Some("—"));
        value_label.set_halign(gtk::Align::End);
        value_label.set_width_chars(7);
        value_label.set_xalign(1.0);
        value_label.add_css_class("metric-value");

        root.append(&name_label);
        root.append(&bar);
        root.append(&value_label);

        let frac = Rc::new(Cell::new(0.0));
        let shown = Rc::new(Cell::new(0.0));

        let d_shown = Rc::clone(&shown);
        bar.set_draw_func(move |_, cr, w, h| draw(cr, w, h, d_shown.get()));

        let (a_frac, a_shown) = (Rc::clone(&frac), Rc::clone(&shown));
        bar.add_tick_callback(move |area, _| {
            let (t, s) = (a_frac.get(), a_shown.get());
            if (t - s).abs() > 0.002 {
                a_shown.set(s + (t - s) * 0.2);
                area.queue_draw();
            }
            glib::ControlFlow::Continue
        });

        Self {
            root,
            bar,
            value_label,
            frac,
        }
    }

    pub fn set(&self, frac: f64, value_text: &str) {
        self.frac.set(frac.clamp(0.0, 1.0));
        self.value_label.set_text(value_text);
        self.bar.queue_draw();
    }
}

fn draw(cr: &gtk::cairo::Context, width: i32, height: i32, frac: f64) {
    let width = f64::from(width);
    let height = f64::from(height);
    let radius = height / 2.0;
    let frac = frac.clamp(0.0, 1.0);

    // rounded track
    rounded_bar(cr, 0.0, width, radius);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
    let _ = cr.fill();

    // faint green/amber/red range bands on the track (meaning, not decoration)
    for (lo, hi) in [
        (0.0, palette::THRESH_WARN),
        (palette::THRESH_WARN, palette::THRESH_CRIT),
        (palette::THRESH_CRIT, 1.0),
    ] {
        rounded_bar(cr, width * lo, width * hi, radius);
        draw::set_source(cr, palette::band(f64::midpoint(lo, hi)), 0.1);
        let _ = cr.fill();
    }

    if frac > 0.0 {
        rounded_bar(cr, 0.0, (width * frac).max(radius * 2.0), radius);
        draw::set_source(cr, palette::band(frac), 0.9);
        let _ = cr.fill();
        // bright leading edge marker
        let x = (width * frac).clamp(radius, width - 1.0);
        cr.set_line_width(2.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
        cr.move_to(x, 2.0);
        cr.line_to(x, height - 2.0);
        let _ = cr.stroke();
    }
}

fn rounded_bar(cr: &gtk::cairo::Context, x0: f64, x1: f64, radius: f64) {
    let top = 0.0;
    let bottom = radius * 2.0;
    cr.new_sub_path();
    cr.arc(x1 - radius, top + radius, radius, -0.5 * PI, 0.5 * PI);
    cr.arc(x0 + radius, bottom - radius, radius, 0.5 * PI, 1.5 * PI);
    cr.close_path();
}
