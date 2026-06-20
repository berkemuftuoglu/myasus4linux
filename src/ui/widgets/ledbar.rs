//! A segmented LED / VU-meter bar: discrete blocks that light up to the value,
//! green through amber to red. Reads like rack-gear metering — denser and far
//! less generic than a smooth fill. Used for per-core load and backlight level.

use gtk::prelude::*;

use super::anim::Animator;
use super::draw;
use crate::ui::palette::{self, Rgb};

const SEGMENTS: usize = 24;

/// How a lit block is coloured.
#[derive(Clone, Copy)]
enum Mode {
    /// Green at the low blocks through amber to red at the top (load, RPM).
    Ramp,
    /// One fixed colour for every lit block (brightness level, where "high"
    /// is not "bad" and a red top would mislead).
    Fixed(Rgb),
}

pub struct LedBar {
    pub root: gtk::Box,
    value_label: gtk::Label,
    anim: Animator,
}

impl LedBar {
    pub fn new(name: &str) -> Self {
        Self::build(name, Mode::Ramp)
    }

    /// A bar whose lit blocks are all one colour, for non-hazard levels.
    pub fn accent(name: &str, color: Rgb) -> Self {
        Self::build(name, Mode::Fixed(color))
    }

    fn build(name: &str, mode: Mode) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 12);

        let name_label = gtk::Label::new(Some(name));
        name_label.set_halign(gtk::Align::Start);
        name_label.set_width_chars(8);
        name_label.set_xalign(0.0);
        name_label.add_css_class("dim-label");

        let bar = gtk::DrawingArea::new();
        bar.set_hexpand(true);
        bar.set_content_height(20);
        bar.set_valign(gtk::Align::Center);

        let value_label = gtk::Label::new(Some("—"));
        value_label.set_halign(gtk::Align::End);
        value_label.set_width_chars(7);
        value_label.set_xalign(1.0);
        value_label.add_css_class("metric-value");

        root.append(&name_label);
        root.append(&bar);
        root.append(&value_label);

        let anim = Animator::new(bar.clone(), 0.22);
        let a = anim.clone();
        bar.set_draw_func(move |_, cr, w, h| draw(cr, w, h, a.shown(), mode));

        Self {
            root,
            value_label,
            anim,
        }
    }

    pub fn set(&self, frac: f64, value_text: &str) {
        self.value_label.set_text(value_text);
        self.anim.set_target(frac.clamp(0.0, 1.0));
    }
}

fn draw(cr: &gtk::cairo::Context, width: i32, height: i32, frac: f64, mode: Mode) {
    let width = f64::from(width);
    let height = f64::from(height);
    let frac = frac.clamp(0.0, 1.0);
    let gap = 3.0;
    let block = (width - gap * (SEGMENTS as f64 - 1.0)) / SEGMENTS as f64;
    let radius = 2.0;

    for i in 0..SEGMENTS {
        let seg = (i as f64 + 0.5) / SEGMENTS as f64;
        let x = i as f64 * (block + gap);
        let color = match mode {
            Mode::Ramp => palette::band(seg),
            Mode::Fixed(rgb) => rgb,
        };
        let lit = seg <= frac;

        if lit {
            // glow halo behind the lit block
            draw::rounded_rect(cr, x - 1.0, -1.0, block + 2.0, height + 2.0, radius);
            draw::set_source(cr, color, 0.22);
            let _ = cr.fill();
        }
        draw::rounded_rect(cr, x, 0.0, block, height, radius);
        draw::set_source(cr, color, if lit { 0.96 } else { 0.10 });
        let _ = cr.fill();
    }
}
