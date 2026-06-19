//! A two-column spec table (property -> value) for static hardware facts.
//! Built as a `gtk::Grid` with `attach`, the GTK idiom for aligned label/value
//! layouts (the same pattern the `ReGreet` greeter uses), dressed as a table
//! with a header row
//! and hairline row rules. Replaces the old info tile grid.

use gtk::prelude::*;

use super::panel::Panel;

pub struct Table {
    pub root: gtk::Box,
}

impl Table {
    pub fn new(title: &str, rows: &[(&str, String)]) -> Self {
        let panel = Panel::new(title);

        let grid = gtk::Grid::new();
        grid.set_hexpand(true);

        grid.attach(&head("Component"), 0, 0, 1, 1);
        grid.attach(&head("Detail"), 1, 0, 1, 1);

        for (i, (key, value)) in rows.iter().enumerate() {
            let row = i32::try_from(i).unwrap_or(0) + 1;
            grid.attach(&cell(key, "spec-key", false), 0, row, 1, 1);
            grid.attach(&cell(value, "spec-value", true), 1, row, 1, 1);
        }

        panel.body.append(&grid);
        Self { root: panel.root }
    }
}

fn head(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("spec-head");
    label.set_halign(gtk::Align::Start);
    label
}

fn cell(text: &str, class: &str, expand: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class(class);
    label.set_halign(gtk::Align::Start);
    label.set_xalign(0.0);
    label.set_hexpand(expand);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label
}
