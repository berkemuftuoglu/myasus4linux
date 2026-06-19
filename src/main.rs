// Panics and indexing are fine in tests; the panic-class lints only guard production paths.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

mod backend;
mod config;
mod ui;

use relm4::RelmApp;
use ui::app::App;

/// Entry point for the myasus4linux application.
///
/// Loads `GResource`s, registers them, and launches the Relm4 application.
fn main() {
    tracing_subscriber::fmt::init();

    // Load and register the compiled GResource bundle (optional for cargo run)
    if let Ok(res) = gio::Resource::load(config::RESOURCES_FILE) {
        gio::resources_register(&res);
    }

    let app = RelmApp::new(config::APP_ID);
    app.run::<App>(());
}
