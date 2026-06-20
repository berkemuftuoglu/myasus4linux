pub mod app;
pub mod builders;
pub mod commit;
pub mod pages;
pub mod palette;
pub mod widgets;

/// Shared dashboard refresh cadence in seconds. Every live page reads this one
/// constant so the screens never poll at different rates.
pub const POLL_SECS: u32 = 2;

/// Debounce before a slider drag commits its privileged write, so one drag is
/// one write. Shared by the charge and screen sliders.
pub const COMMIT_DEBOUNCE_MS: u64 = 400;

/// Feedback a page sends up to the app: a failure/warning (toasted at warn) or a
/// success notice (toasted at info). Shared so each page doesn't redefine it.
#[derive(Debug)]
pub enum PageMsg {
    Error(String),
    Notice(String),
}

/// Run a blocking hardware read on a worker thread and deliver the message it
/// produces back to the component's input. Keeping sysfs reads off the GTK main
/// thread is needed on every live page, so the pattern lives here once instead
/// of being copied per page.
pub fn offload<M: Send + 'static>(
    input: &relm4::Sender<M>,
    job: impl FnOnce() -> M + Send + 'static,
) {
    let input = input.clone();
    std::thread::spawn(move || {
        let _ = input.send(job());
    });
}

/// Install a visibility-gated recurring poll: every [`POLL_SECS`], while `root`
/// is mapped (its page is the visible stack child), send the message `make()`
/// produces. Stops when the widget is dropped. Hidden pages then don't poll
/// sysfs or spawn workers -- with the idle animation tick fixed, that per-page
/// background polling is the dashboard's remaining idle cost. Pairs with
/// [`offload`] and replaces the ticker boilerplate each page used to repeat.
pub fn poll<M: 'static>(
    root: &impl gtk::prelude::IsA<gtk::Widget>,
    sender: &relm4::Sender<M>,
    make: impl Fn() -> M + 'static,
) {
    use gtk::prelude::{ObjectExt, WidgetExt};
    let weak = root.as_ref().downgrade();
    let sender = sender.clone();
    glib::timeout_add_seconds_local(POLL_SECS, move || {
        let Some(widget) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if widget.is_mapped() {
            let _ = sender.send(make());
        }
        glib::ControlFlow::Continue
    });
}

/// Schedule a slider's deferred commit: after [`COMMIT_DEBOUNCE_MS`] with no
/// further movement, deliver `make(seq)` so the page runs its privileged write
/// once. Pairs with [`commit::DebouncedCommit`], which hands out the `seq`, so a
/// page no longer hand-rolls the same glib timeout per slider.
pub fn debounce_commit<M: 'static>(
    sender: &relm4::Sender<M>,
    seq: u32,
    make: impl Fn(u32) -> M + 'static,
) {
    let sender = sender.clone();
    glib::timeout_add_local(
        std::time::Duration::from_millis(COMMIT_DEBOUNCE_MS),
        move || {
            let _ = sender.send(make(seq));
            glib::ControlFlow::Break
        },
    );
}
