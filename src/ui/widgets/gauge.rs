//! A live instrument-style radial gauge: gradient-swept arc, sweeping needle
//! and hub, tick scale, glow. Drawn with Cairo and eased between values.

use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

use gtk::prelude::*;

use super::anim::Animator;
use super::draw;
use crate::ui::palette::{self, Rgb};

/// How the value arc is coloured.
#[derive(Clone, Copy)]
pub enum Accent {
    /// Green at low values through amber to red at the top (load/temperature).
    ByValue,
    /// A fixed colour regardless of value (battery charge, health).
    Fixed(Rgb),
}

impl Accent {
    fn color_at(self, frac: f64) -> Rgb {
        match self {
            Accent::ByValue => palette::ramp(frac),
            Accent::Fixed(rgb) => rgb,
        }
    }
}

pub struct Gauge {
    pub area: gtk::DrawingArea,
    anim: Animator,
    big: Rc<RefCell<String>>,
    sub: Rc<RefCell<String>>,
}

impl Gauge {
    pub fn new(size: i32, accent: Accent) -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_width(size);
        area.set_content_height(size);

        let big = Rc::new(RefCell::new(String::new()));
        let sub = Rc::new(RefCell::new(String::new()));

        let anim = Animator::new(area.clone(), 0.18);
        let (a, d_big, d_sub) = (anim.clone(), Rc::clone(&big), Rc::clone(&sub));
        area.set_draw_func(move |_, cr, w, h| {
            draw_gauge(
                cr,
                w,
                h,
                a.shown(),
                accent,
                &d_big.borrow(),
                &d_sub.borrow(),
            );
        });

        Self {
            area,
            anim,
            big,
            sub,
        }
    }

    /// `frac` is 0.0..=1.0 (fill amount); `big` is the centre value, `sub` the label.
    pub fn set(&self, frac: f64, big: &str, sub: &str) {
        big.clone_into(&mut self.big.borrow_mut());
        sub.clone_into(&mut self.sub.borrow_mut());
        self.anim.set_target(frac.clamp(0.0, 1.0));
    }
}

fn draw_gauge(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    frac: f64,
    accent: Accent,
    big: &str,
    sub: &str,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    let cx = width / 2.0;
    let cy = height / 2.0;
    let radius = width.min(height) / 2.0 - 18.0;
    let start = 0.75 * PI;
    let span = 1.5 * PI;
    let frac = frac.clamp(0.0, 1.0);
    let value_color = accent.color_at(frac);

    // base track
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_width(10.0);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
    cr.arc(cx, cy, radius, start, start + span);
    let _ = cr.stroke();

    // soft glow under the value arc
    if frac > 0.0 {
        cr.set_line_width(20.0);
        draw::set_source(cr, value_color, 0.14);
        cr.arc(cx, cy, radius, start, start + span * frac);
        let _ = cr.stroke();
    }

    // value arc, drawn in short segments that interpolate colour along the
    // sweep (Cairo has no conic gradient, so we fake it segment by segment)
    if frac > 0.0 {
        cr.set_line_cap(gtk::cairo::LineCap::Butt);
        cr.set_line_width(10.0);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "segment count is a small bounded positive (<= ~80)"
        )]
        let segments = ((span * frac) / 0.06).ceil().max(1.0) as i32;
        for i in 0..segments {
            let f0 = frac * f64::from(i) / f64::from(segments);
            let f1 = frac * f64::from(i + 1) / f64::from(segments);
            draw::set_source(cr, accent.color_at(f1), 1.0);
            cr.arc(cx, cy, radius, start + span * f0, start + span * f1 + 0.01);
            let _ = cr.stroke();
        }
    }

    draw_scale(cr, cx, cy, radius, start, span);

    // needle + hub
    let a = start + span * frac;
    let (co, si) = (a.cos(), a.sin());
    let (px, py) = (-si, co);
    let tip = radius - 7.0;
    let base = (radius * 0.06).max(4.0);
    cr.move_to(cx + co * tip, cy + si * tip);
    cr.line_to(cx + px * base, cy + py * base);
    cr.line_to(cx - px * base, cy - py * base);
    cr.close_path();
    draw::set_source(cr, value_color, 0.95);
    let _ = cr.fill();
    cr.arc(cx, cy, base + 3.0, 0.0, 2.0 * PI);
    cr.set_source_rgba(0.04, 0.06, 0.09, 1.0);
    let _ = cr.fill();
    cr.set_line_width(2.0);
    cr.arc(cx, cy, base + 3.0, 0.0, 2.0 * PI);
    draw::set_source(cr, value_color, 0.9);
    let _ = cr.stroke();

    // big value sits in the open bottom of the dial, clear of the needle
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.98);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Bold,
    );
    cr.set_font_size(radius * 0.40);
    draw::centered_text(cr, big, cx, cy + radius * 0.50);

    draw::set_source(cr, value_color, 0.9);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Normal,
    );
    cr.set_font_size((radius * 0.14).max(9.0));
    draw::centered_text(cr, sub, cx, cy + radius * 0.74);
}

/// Tick marks (majors every 25%) and the 0 / 50 / 100 scale numbers.
fn draw_scale(cr: &gtk::cairo::Context, cx: f64, cy: f64, radius: f64, start: f64, span: f64) {
    cr.set_line_cap(gtk::cairo::LineCap::Butt);
    for i in 0..=20 {
        let a = start + span * f64::from(i) / 20.0;
        let (co, si) = (a.cos(), a.sin());
        let major = i % 5 == 0;
        let inner = radius - if major { 15.0 } else { 8.0 };
        cr.set_line_width(if major { 1.8 } else { 1.0 });
        cr.set_source_rgba(0.6, 0.78, 0.85, if major { 0.55 } else { 0.28 });
        cr.move_to(cx + co * inner, cy + si * inner);
        cr.line_to(cx + co * (radius - 4.0), cy + si * (radius - 4.0));
        let _ = cr.stroke();
    }

    cr.set_source_rgba(0.55, 0.72, 0.8, 0.6);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Normal,
    );
    cr.set_font_size((radius * 0.13).max(8.0));
    for (t, label) in [(0.0, "0"), (0.5, "50"), (1.0, "100")] {
        let a = start + span * t;
        let lr = radius * 0.72;
        draw::centered_text(cr, label, cx + a.cos() * lr, cy + a.sin() * lr + 3.0);
    }
}
