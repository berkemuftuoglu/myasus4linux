use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("no battery with a charge-threshold control was found")]
    NoBattery,
    #[error("no supported performance-profile interface was found")]
    NoFanControl,
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
            HelperError::Write { .. }
            | HelperError::NoBattery
            | HelperError::NoFanControl
            | HelperError::Polkit(_)
            | HelperError::Subject(_) => zbus::fdo::Error::Failed(e.to_string()),
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
    let (target, payload) = resolve_write(op)?;
    // Defence in depth: the path is never caller-supplied (fixed per Op, or built
    // from the kernel's own enumeration), but a privileged writer must still
    // refuse anything that is not an absolute /sys attribute, so a bug can't turn
    // into an arbitrary write.
    if !target.is_absolute() || !target.starts_with("/sys/") {
        return Err(HelperError::Write {
            path: target.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "refusing to write a non-/sys path",
            ),
        });
    }
    let value = op.raw_value();
    let persist = matches!(op, Op::ChargeThreshold(_));
    // Run the sysfs (and state-file) writes on a blocking thread so they never
    // stall the async D-Bus executor. The outer map_err handles a panicked
    // worker; the inner one handles the write itself failing.
    let write_path = target.clone();
    let to_write = payload.clone();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        // Skip when the attribute already holds this exact value (int or string),
        // so we don't churn the EC rewriting an unchanged setting.
        let already = std::fs::read_to_string(&write_path)
            .ok()
            .is_some_and(|s| s.trim() == to_write);
        if !already {
            std::fs::write(&write_path, &to_write)?;
        }
        if persist {
            persist_charge_threshold(value);
        }
        Ok(())
    })
    .await
    .map_err(|join| HelperError::Write {
        path: target.display().to_string(),
        source: std::io::Error::other(join),
    })?
    .map_err(|source| HelperError::Write {
        path: target.display().to_string(),
        source,
    })?;
    tracing::info!("wrote {payload} to {}", target.display());
    Ok(())
}

/// The sysfs attribute an `Op` writes and the exact bytes to write. Fixed for
/// keyboard; the charge threshold resolves the enumerated battery; the fan
/// profile chooses the live interface (ASUS WMI int, else firmware
/// `platform_profile` string). The value never carries a path.
fn resolve_write(op: Op) -> Result<(PathBuf, String), HelperError> {
    match op {
        Op::ChargeThreshold(v) => {
            let p = myasus_core::charge_threshold_path(Path::new(myasus_core::POWER_SUPPLY_ROOT))
                .ok_or(HelperError::NoBattery)?;
            Ok((p, v.to_string()))
        }
        Op::KeyboardBacklight(v) => {
            Ok((PathBuf::from(myasus_core::KBD_BACKLIGHT_PATH), v.to_string()))
        }
        Op::FanProfile(v) => fan_profile_write(v),
    }
}

/// Prefer the ASUS WMI integer interface; fall back to the kernel-standard
/// firmware `platform_profile` (string token, validated against the machine's
/// advertised choices).
fn fan_profile_write(value: u8) -> Result<(PathBuf, String), HelperError> {
    if Path::new(myasus_core::FAN_PROFILE_PATH).exists() {
        return Ok((PathBuf::from(myasus_core::FAN_PROFILE_PATH), value.to_string()));
    }
    let token = myasus_core::platform_profile_token(value).ok_or(HelperError::NoFanControl)?;
    if Path::new(myasus_core::PLATFORM_PROFILE_PATH).exists() && platform_profile_supports(token) {
        Ok((PathBuf::from(myasus_core::PLATFORM_PROFILE_PATH), token.to_owned()))
    } else {
        Err(HelperError::NoFanControl)
    }
}

fn platform_profile_supports(token: &str) -> bool {
    std::fs::read_to_string(myasus_core::PLATFORM_PROFILE_CHOICES_PATH)
        .is_ok_and(|s| s.split_whitespace().any(|c| c == token))
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
///
/// Atomic: write a sibling temp then rename over the target. A crash or
/// power-loss mid-write can then never leave a half-written threshold that the
/// daemon would read back as garbage on the next boot.
fn write_state(path: &Path, value: u8) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, value.to_string())?;
    std::fs::rename(&tmp, path)
}

/// Read a small unsigned value from a sysfs attribute, `None` if missing or
/// unparseable. Used to skip rewriting a value the EC already holds.
fn read_sysfs_u8(path: &Path) -> Option<u8> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
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
    let Some(target) = myasus_core::charge_threshold_path(Path::new(myasus_core::POWER_SUPPLY_ROOT))
    else {
        tracing::warn!("no battery charge-threshold control found; cannot restore");
        return;
    };
    if read_sysfs_u8(&target) == Some(value) {
        tracing::info!("charge threshold already {value} on startup, nothing to restore");
        return;
    }
    match std::fs::write(&target, value.to_string()) {
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
        // The atomic write must not leave its temp file behind.
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn write_state_overwrites_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("charge_threshold");
        write_state(&path, 60).unwrap();
        write_state(&path, 85).unwrap();
        assert_eq!(parse_persisted(&std::fs::read_to_string(&path).unwrap()), Some(85));
        assert!(!path.with_extension("tmp").exists());
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
