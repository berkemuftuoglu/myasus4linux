//! A Grafana-style panel: a framed box with a thin header strip (title on the
//! left, an optional live value in the corner) and a body you fill with a
//! gauge, chart, meter matrix, or controls. This is the one building block
//! every dashboard screen is composed from.

use gtk::prelude::*;

pub struct Panel {
    pub root: gtk::Box,
    pub body: gtk::Box,
    corner: gtk::Label,
}

impl Panel {
    /// A panel whose body stacks its children vertically.
    pub fn new(title: &str) -> Self {
        Self::with_orientation(title, gtk::Orientation::Vertical)
    }

    pub fn with_orientation(title: &str, orientation: gtk::Orientation) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("panel");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("panel-header");

        let title_label = gtk::Label::new(Some(title));
        title_label.add_css_class("panel-title");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_hexpand(true);
        title_label.set_xalign(0.0);

        let corner = gtk::Label::new(None);
        corner.add_css_class("panel-corner");
        corner.set_halign(gtk::Align::End);

        header.append(&title_label);
        header.append(&corner);

        let body = gtk::Box::new(orientation, 12);
        body.add_css_class("panel-body");
        body.set_hexpand(true);

        root.append(&header);
        root.append(&body);

        Self { root, body, corner }
    }

    /// The small live readout shown at the right of the header (e.g. "61 °C").
    pub fn set_corner(&self, text: &str) {
        self.corner.set_text(text);
    }

    /// A panel wrapping a single drawn widget (gauge, battery cell, chart).
    /// `min_width` lets the `FlowBox` grid decide when to wrap; `center` centres
    /// the body for square dials, otherwise the child fills the width.
    pub fn metric(
        title: &str,
        child: &impl IsA<gtk::Widget>,
        min_width: i32,
        center: bool,
    ) -> gtk::Box {
        let panel = Self::new(title);
        if center {
            panel.body.set_halign(gtk::Align::Center);
        } else {
            child.set_hexpand(true);
        }
        panel.body.append(child);
        panel.root.set_size_request(min_width, -1);
        panel.root
    }
}
