//! Shared Cairo primitives used by every custom widget, so the rounded-rect
//! and centered-text routines live in one place instead of a copy per file.

use std::f64::consts::PI;

use crate::ui::palette::Rgb;

/// Set the Cairo source to a palette colour at the given alpha.
pub fn set_source(cr: &gtk::cairo::Context, color: Rgb, alpha: f64) {
    cr.set_source_rgba(color.r, color.g, color.b, alpha);
}

/// Trace a rounded rectangle as the current path (caller fills or strokes).
pub fn rounded_rect(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let radius = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - radius, y + radius, radius, -0.5 * PI, 0.0);
    cr.arc(x + w - radius, y + h - radius, radius, 0.0, 0.5 * PI);
    cr.arc(x + radius, y + h - radius, radius, 0.5 * PI, PI);
    cr.arc(x + radius, y + radius, radius, PI, 1.5 * PI);
    cr.close_path();
}

/// Draw text horizontally centered on `cx`, baseline at `y`.
pub fn centered_text(cr: &gtk::cairo::Context, text: &str, cx: f64, y: f64) {
    if let Ok(ext) = cr.text_extents(text) {
        cr.move_to(cx - ext.width() / 2.0 - ext.x_bearing(), y);
        let _ = cr.show_text(text);
    }
}
