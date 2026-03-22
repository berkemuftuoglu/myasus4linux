mod backend;
mod config;
mod ui;

use anyhow::Result;
use relm4::RelmApp;
use ui::app::App;

/// Entry point for the myasus4linux application.
///
/// Loads GResources, registers them, and launches the Relm4 application.
fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Load and register the compiled GResource bundle (optional for cargo run)
    if let Ok(res) = gio::Resource::load(config::RESOURCES_FILE) {
        gio::resources_register(&res);
    }

    let app = RelmApp::new(config::APP_ID);
    app.run::<App>(());

    Ok(())
}
