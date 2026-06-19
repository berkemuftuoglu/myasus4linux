use std::collections::HashMap;
use std::path::Path;

use myasus_core::Op;
use zbus::message::Header;
use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

const ACTION_CHARGE: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetChargeThreshold";
const ACTION_FAN: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetPerformanceProfile";
const ACTION_KBD: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetKeyboardBacklight";

/// Root-owned state, provided by the unit's `StateDirectory=myasus4linux`. The
/// charge limit is recorded here so it survives a reboot without a GUI or any
/// `pkexec` install step.
const CHARGE_STATE_FILE: &str = "/var/lib/myasus4linux/charge_threshold";

#[derive(Debug, thiserror::Error)]
enum HelperError {
    #[error("not authorized")]
    NotAuthorized,
    #[error("{0}")]
    Validate(#[from] myasus_core::ValidateError),
    #[error("failed to write {path}: {source}")]
    Write {
        path: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("authorization check failed: {0}")]
    Polkit(#[from] zbus::Error),
    #[error("could not identify caller: {0}")]
    Subject(#[from] zbus_polkit::Error),
}

impl From<HelperError> for zbus::fdo::Error {
    fn from(e: HelperError) -> Self {
        match e {
            HelperError::NotAuthorized => zbus::fdo::Error::AccessDenied(e.to_string()),
            HelperError::Validate(_) => zbus::fdo::Error::InvalidArgs(e.to_string()),
            HelperError::Write { .. } | HelperError::Polkit(_) | HelperError::Subject(_) => {
                zbus::fdo::Error::Failed(e.to_string())
            }
        }
    }
}

pub struct Helper;

#[zbus::interface(name = "io.github.berkmuftuoglu.MyAsus4Linux.Helper")]
impl Helper {
    async fn set_charge_threshold(
        &self,
        value: u8,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        Ok(apply(
            connection,
            &header,
            ACTION_CHARGE,
            Op::ChargeThreshold(value),
        )
        .await?)
    }

    async fn set_fan_profile(
        &self,
        value: u8,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        Ok(apply(connection, &header, ACTION_FAN, Op::FanProfile(value)).await?)
    }

    async fn set_keyboard_backlight(
        &self,
        value: u8,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        Ok(apply(
            connection,
            &header,
            ACTION_KBD,
            Op::KeyboardBacklight(value),
        )
        .await?)
    }
}

/// Authorise the caller for `action_id`, then validate and perform `op`.
async fn apply(
    connection: &zbus::Connection,
    header: &Header<'_>,
    action_id: &str,
    op: Op,
) -> Result<(), HelperError> {
    authorize(connection, header, action_id).await?;
    op.validate()?;
    let path = op.path();
    // Defence in depth: the path is a fixed constant per Op and never caller-
    // supplied, but a privileged writer must still refuse anything that is not an
    // absolute /sys attribute, so a future Op variant can't become an arbitrary
    // write. Empty or relative paths fail this check.
    if !std::path::Path::new(path).is_absolute() {
        return Err(HelperError::Write {
            path,
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "refusing to write a non-absolute path",
            ),
        });
    }
    let value = op.raw_value();
    let persist = matches!(op, Op::ChargeThreshold(_));
    // Run the sysfs (and state-file) writes on a blocking thread so they never
    // stall the async D-Bus executor. The outer map_err handles a panicked
    // worker; the inner one handles the write itself failing.
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        std::fs::write(path, value.to_string())?;
        if persist {
            persist_charge_threshold(value);
        }
        Ok(())
    })
    .await
    .map_err(|join| HelperError::Write {
        path,
        source: std::io::Error::other(join),
    })?
    .map_err(|source| HelperError::Write { path, source })?;
    tracing::info!("wrote {value} to {path}");
    Ok(())
}

/// Record the charge limit for [`restore_charge_threshold`]. Best-effort: a
/// failure to persist must never fail the write the user just authorised.
fn persist_charge_threshold(value: u8) {
    if let Err(e) = write_state(Path::new(CHARGE_STATE_FILE), value) {
        tracing::warn!("could not persist charge threshold: {e}");
    }
}

/// Write the charge value to `path`, creating its parent directory. Path is
/// injectable so the round-trip can be exercised without touching `/var/lib`.
fn write_state(path: &Path, value: u8) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value.to_string())
}

/// Parse and range-check persisted state. None for malformed or out-of-range
/// contents. Pure, so it is unit-tested directly.
fn parse_persisted(raw: &str) -> Option<u8> {
    let value = raw.trim().parse::<u8>().ok()?;
    Op::ChargeThreshold(value).validate().ok()?;
    Some(value)
}

/// Re-apply the persisted charge limit at daemon startup, so it is restored on
/// boot before any GUI runs. Silent when nothing has ever been saved.
pub fn restore_charge_threshold() {
    let Ok(raw) = std::fs::read_to_string(CHARGE_STATE_FILE) else {
        return; // nothing saved yet
    };
    let Some(value) = parse_persisted(&raw) else {
        tracing::warn!("ignoring malformed or out-of-range persisted charge threshold {raw:?}");
        return;
    };
    let op = Op::ChargeThreshold(value);
    match std::fs::write(op.path(), op.raw_value().to_string()) {
        Ok(()) => tracing::info!("restored charge threshold {value} on startup"),
        Err(e) => tracing::warn!("could not restore charge threshold: {e}"),
    }
}

/// Ask polkit whether the caller (identified by their bus message) may perform
/// `action_id`. Interactive auth is allowed so the user gets a prompt.
async fn authorize(
    connection: &zbus::Connection,
    header: &Header<'_>,
    action_id: &str,
) -> Result<(), HelperError> {
    let authority = AuthorityProxy::new(connection).await?;
    let subject = Subject::new_for_message_header(header)?;
    let result = authority
        .check_authorization(
            &subject,
            action_id,
            &HashMap::new(),
            CheckAuthorizationFlags::AllowUserInteraction.into(),
            "",
        )
        .await?;
    if result.is_authorized {
        Ok(())
    } else {
        Err(HelperError::NotAuthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_state_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/charge_threshold");
        write_state(&path, 70).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(parse_persisted(&raw), Some(70));
    }

    #[test]
    fn parse_persisted_accepts_in_range() {
        assert_eq!(parse_persisted("80"), Some(80));
        assert_eq!(parse_persisted(" 60\n"), Some(60));
    }

    #[test]
    fn parse_persisted_rejects_out_of_range_and_garbage() {
        assert_eq!(parse_persisted("39"), None); // below the 40 floor
        assert_eq!(parse_persisted("250"), None); // parses as u8 but above 100
        assert_eq!(parse_persisted("xyz"), None);
        assert_eq!(parse_persisted(""), None);
    }
}
