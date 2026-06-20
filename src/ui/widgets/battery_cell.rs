//! The battery shown as an actual cell: a rounded body with a terminal nub,
//! filling left-to-right by charge (green when healthy, amber low, red
//! critical), with a lightning bolt when charging. Thematic, not a generic dial.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use super::anim::Animator;
use super::draw;
use crate::ui::palette;

pub struct BatteryCell {
    pub area: gtk::DrawingArea,
    anim: Animator,
    charging: Rc<Cell<bool>>,
    big: Rc<RefCell<String>>,
    sub: Rc<RefCell<String>>,
}

impl BatteryCell {
    pub fn new(size: i32) -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_width(size);
        area.set_content_height(size);

        let charging = Rc::new(Cell::new(false));
        let big = Rc::new(RefCell::new(String::new()));
        let sub = Rc::new(RefCell::new(String::new()));

        let anim = Animator::new(area.clone(), 0.18);
        let (a, d_chg, d_big, d_sub) = (
            anim.clone(),
            Rc::clone(&charging),
            Rc::clone(&big),
            Rc::clone(&sub),
        );
        area.set_draw_func(move |_, cr, w, h| {
            draw(cr, w, h, a.shown(), d_chg.get(), &d_big.borrow(), &d_sub.borrow());
        });

        Self {
            area,
            anim,
            charging,
            big,
            sub,
        }
    }

    pub fn set(&self, frac: f64, charging: bool, big: &str, sub: &str) {
        self.charging.set(charging);
        big.clone_into(&mut self.big.borrow_mut());
        sub.clone_into(&mut self.sub.borrow_mut());
        self.anim.set_target(frac.clamp(0.0, 1.0));
    }
}

fn draw(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    frac: f64,
    charging: bool,
    big: &str,
    sub: &str,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    let frac = frac.clamp(0.0, 1.0);
    let color = palette::charge(frac);

    // battery body
    let bx = width * 0.14;
    let by = height * 0.28;
    let bw = width * 0.62;
    let bh = height * 0.30;
    let r = bh * 0.18;

    // outer shell glow
    draw::rounded_rect(cr, bx - 2.0, by - 2.0, bw + 4.0, bh + 4.0, r + 2.0);
    draw::set_source(cr, color, 0.12);
    let _ = cr.fill();

    // shell outline
    cr.set_line_width(2.5);
    draw::rounded_rect(cr, bx, by, bw, bh, r);
    cr.set_source_rgba(0.7, 0.85, 0.9, 0.55);
    let _ = cr.stroke();

    // terminal nub
    let nub_w = width * 0.04;
    draw::rounded_rect(
        cr,
        bx + bw + 1.5,
        by + bh * 0.28,
        nub_w,
        bh * 0.44,
        nub_w * 0.4,
    );
    cr.set_source_rgba(0.7, 0.85, 0.9, 0.55);
    let _ = cr.fill();

    // fill
    let pad = 3.5;
    let fill_w = (bw - pad * 2.0) * frac;
    if fill_w > 0.0 {
        draw::rounded_rect(cr, bx + pad, by + pad, fill_w, bh - pad * 2.0, r * 0.6);
        draw::set_source(cr, color, 0.9);
        let _ = cr.fill();
        // top gloss
        draw::rounded_rect(
            cr,
            bx + pad,
            by + pad,
            fill_w,
            (bh - pad * 2.0) * 0.4,
            r * 0.6,
        );
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.12);
        let _ = cr.fill();
    }

    // charging bolt
    if charging {
        let cxp = bx + bw / 2.0;
        let cyp = by + bh / 2.0;
        let s = bh * 0.42;
        cr.move_to(cxp + s * 0.15, cyp - s);
        cr.line_to(cxp - s * 0.45, cyp + s * 0.15);
        cr.line_to(cxp + s * 0.02, cyp + s * 0.15);
        cr.line_to(cxp - s * 0.15, cyp + s);
        cr.line_to(cxp + s * 0.45, cyp - s * 0.15);
        cr.line_to(cxp - s * 0.02, cyp - s * 0.15);
        cr.close_path();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        let _ = cr.fill();
    }

    // big value below the cell
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.98);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Bold,
    );
    cr.set_font_size(height * 0.22);
    draw::centered_text(cr, big, width / 2.0, height * 0.78);

    draw::set_source(cr, color, 0.9);
    cr.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Normal,
    );
    cr.set_font_size(height * 0.085);
    draw::centered_text(cr, sub, width / 2.0, height * 0.90);
}
