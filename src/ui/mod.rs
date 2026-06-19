pub mod app;
pub mod builders;
pub mod pages;
pub mod widgets;

/// Shared dashboard refresh cadence in seconds. Every live page reads this one
/// constant so the screens never poll at different rates.
pub const POLL_SECS: u32 = 2;

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
