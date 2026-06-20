//! Frame-clock easing for the drawn widgets.
//!
//! A bare `add_tick_callback` that returns `Continue` runs every frame for the
//! widget's whole life, keeping the frame clock awake at 60Hz even when nothing
//! moves -- a real idle CPU/battery cost on a dashboard full of gauges. This
//! eases one `0.0..=1.0` value toward a target, then returns `Break` once it
//! converges and re-arms only when a new target arrives, so a still dashboard
//! lets the clock sleep. Shared by every animated widget instead of copy-pasted.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

const EPSILON: f64 = 0.002;

#[derive(Clone)]
pub struct Animator {
    target: Rc<Cell<f64>>,
    shown: Rc<Cell<f64>>,
    animating: Rc<Cell<bool>>,
    area: gtk::DrawingArea,
    factor: f64,
}

impl Animator {
    /// `factor` is the per-frame fraction of the remaining distance to ease.
    pub fn new(area: gtk::DrawingArea, factor: f64) -> Self {
        Self {
            target: Rc::new(Cell::new(0.0)),
            shown: Rc::new(Cell::new(0.0)),
            animating: Rc::new(Cell::new(false)),
            area,
            factor,
        }
    }

    /// The eased value the draw function should render.
    pub fn shown(&self) -> f64 {
        self.shown.get()
    }

    /// Point the easing at a new target and redraw. Arms the frame clock only if
    /// it isn't already running and there is distance left to cover.
    pub fn set_target(&self, target: f64) {
        self.target.set(target);
        self.area.queue_draw();
        if (self.target.get() - self.shown.get()).abs() > EPSILON {
            self.arm();
        }
    }

    fn arm(&self) {
        // replace returns the previous value; if a tick is already running, bail.
        if self.animating.replace(true) {
            return;
        }
        let target = Rc::clone(&self.target);
        let shown = Rc::clone(&self.shown);
        let animating = Rc::clone(&self.animating);
        let factor = self.factor;
        self.area.add_tick_callback(move |area, _| {
            let (t, s) = (target.get(), shown.get());
            if (t - s).abs() <= EPSILON {
                shown.set(t);
                area.queue_draw();
                animating.set(false);
                return glib::ControlFlow::Break;
            }
            shown.set(s + (t - s) * factor);
            area.queue_draw();
            glib::ControlFlow::Continue
        });
    }
}
