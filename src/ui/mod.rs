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
