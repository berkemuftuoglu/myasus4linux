//! Client side of the privileged write path: a blocking proxy to `myasusd`.
//!
//! The GUI never writes the root-owned sysfs controls itself. It hands the typed
//! value to the daemon, which authorises the caller with polkit and performs the
//! write. These calls block, so the backend runs them on a worker thread.

use std::sync::OnceLock;

use super::error::BackendError;

#[zbus::proxy(
    interface = "io.github.berkmuftuoglu.MyAsus4Linux.Helper",
    default_service = "io.github.berkmuftuoglu.MyAsus4Linux.Helper",
    default_path = "/io/github/berkmuftuoglu/MyAsus4Linux/Helper"
)]
trait Helper {
    fn set_charge_threshold(&self, value: u8) -> zbus::Result<()>;
    fn set_fan_profile(&self, value: u8) -> zbus::Result<()>;
    fn set_keyboard_backlight(&self, value: u8) -> zbus::Result<()>;
}

/// One shared system-bus connection for the whole process. Opening a connection
/// performs a bus handshake, so caching it keeps a burst of writes -- dragging
/// the charge slider fires one per step -- from reconnecting on every call.
fn connection() -> Result<&'static zbus::blocking::Connection, BackendError> {
    static CONN: OnceLock<zbus::blocking::Connection> = OnceLock::new();
    if let Some(conn) = CONN.get() {
        return Ok(conn);
    }
    let conn = zbus::blocking::Connection::system().map_err(BackendError::Daemon)?;
    Ok(CONN.get_or_init(|| conn))
}

fn proxy() -> Result<HelperProxyBlocking<'static>, BackendError> {
    HelperProxyBlocking::new(connection()?).map_err(BackendError::Daemon)
}

pub fn set_charge_threshold(value: u8) -> Result<(), BackendError> {
    proxy()?
        .set_charge_threshold(value)
        .map_err(BackendError::Daemon)
}

pub fn set_fan_profile(value: u8) -> Result<(), BackendError> {
    proxy()?
        .set_fan_profile(value)
        .map_err(BackendError::Daemon)
}

pub fn set_keyboard_backlight(value: u8) -> Result<(), BackendError> {
    proxy()?
        .set_keyboard_backlight(value)
        .map_err(BackendError::Daemon)
}
