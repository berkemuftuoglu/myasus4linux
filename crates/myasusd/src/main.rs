#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

//! `myasusd` -- the privileged D-Bus system daemon for myasus4linux.
//!
//! Runs as root, owns the well-known name on the system bus, and is the only
//! thing that writes the ASUS hardware controls. The GUI talks to it instead of
//! escalating with `pkexec` or relying on a world-writable sysfs file. Each
//! method authorises the caller with polkit, then validates and writes through
//! the `myasus_core::Op` contract -- callers never supply a path.

mod helper;

use helper::Helper;
use myasus_core::{DBUS_NAME, DBUS_PATH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Re-apply all saved settings first, so a boot-time start restores them
    // before anything else; then serve requests for the rest of the session.
    helper::restore_state();

    // Held (not `_`) so the connection lives until shutdown; dropping it would
    // release the bus name.
    let _connection = zbus::connection::Builder::system()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_PATH, Helper)?
        .build()
        .await?;

    tracing::info!("myasusd up, owning {DBUS_NAME} at {DBUS_PATH}");

    wait_for_shutdown().await;
    tracing::info!("shutting down");
    Ok(())
}

/// Block until the service manager stops us (SIGTERM) or we are interrupted
/// (Ctrl-C), so `systemctl stop` exits cleanly instead of being killed.
async fn wait_for_shutdown() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut term = match signal(SignalKind::terminate()) {
        Ok(stream) => stream,
        Err(e) => {
            tracing::warn!("cannot watch for SIGTERM ({e}); running until killed");
            std::future::pending::<()>().await;
            return;
        }
    };
    tokio::select! {
        _ = term.recv() => {}
        result = tokio::signal::ctrl_c() => {
            if let Err(e) = result {
                tracing::warn!("ctrl_c watch failed: {e}");
            }
        }
    }
}
