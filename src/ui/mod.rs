pub mod app;
pub mod battery_cell;
pub mod battery_page;
pub mod chart;
pub mod cores;
pub mod cpu_page;
pub mod draw;
pub mod fan_page;
pub mod gauge;
pub mod info_page;
pub mod keyboard_page;
pub mod ledbar;
pub mod meter;
pub mod overview;
pub mod panel;
pub mod stat;
pub mod table;
pub mod zones;

/// Shared dashboard refresh cadence in seconds. Every live page reads this one
/// constant so the screens never poll at different rates.
pub const POLL_SECS: u32 = 2;

/// Run a blocking hardware read on a worker thread and deliver the message it
/// produces back to the component's input. Keeping sysfs reads off the GTK main
/// thread is needed on every live page, so the pattern lives here once instead
/// of being copied per page.
pub fn offload<M: Send + 'static>(input: &relm4::Sender<M>, job: impl FnOnce() -> M + Send + 'static) {
    let input = input.clone();
    std::thread::spawn(move || {
        let _ = input.send(job());
    });
}
