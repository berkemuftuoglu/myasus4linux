//! Client side of the privileged write path: a blocking proxy to `myasusd`.
//!
//! The GUI never writes the root-owned sysfs controls itself. It hands the typed
//! value to the daemon, which authorises the caller with polkit and performs the
//! write. These calls block, so the backend runs them on a worker thread.

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

fn proxy() -> Result<HelperProxyBlocking<'static>, BackendError> {
    let connection = zbus::blocking::Connection::system().map_err(BackendError::Daemon)?;
    HelperProxyBlocking::new(&connection).map_err(BackendError::Daemon)
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
