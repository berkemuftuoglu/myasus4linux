use std::collections::HashMap;
use std::path::{Path, PathBuf};

use myasus_core::{DaemonState, Op};
use zbus::message::Header;
use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

/// Minimal client proxy for logind's sleep signal, used to re-apply the settings
/// the EC forgets across suspend.
#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login1Manager {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

const ACTION_CHARGE: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetChargeThreshold";
const ACTION_FAN: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetPerformanceProfile";
const ACTION_KBD: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetKeyboardBacklight";

/// Root-owned state, provided by the unit's `StateDirectory=myasus4linux`. The
/// persisted settings (charge limit, fan profile, keyboard backlight) live here
/// so they survive a reboot and are re-applied on resume, with no GUI involved.
/// A legacy bare-number charge file is still read transparently.
const STATE_FILE: &str = "/var/lib/myasus4linux/state";

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
    // Do the privileged write (and persist) on a blocking thread so neither
    // stalls the async D-Bus executor. The map_err handles a panicked worker;
    // the inner `?`s handle the write/persist themselves failing.
    tokio::task::spawn_blocking(move || -> Result<(), HelperError> {
        perform_write(op)?;
        persist_op(op);
        Ok(())
    })
    .await
    .map_err(|join| HelperError::Write {
        path: "worker".to_owned(),
        source: std::io::Error::other(join),
    })??;
    Ok(())
}

/// Resolve the target + payload, refuse anything that is not an absolute /sys
/// attribute, skip a redundant write, then write. Synchronous: called on a
/// blocking thread from `apply`, and directly at startup/resume from
/// [`restore_state`].
fn perform_write(op: Op) -> Result<(), HelperError> {
    let (target, payload) = resolve_write(op)?;
    // Defence in depth: the path is never caller-supplied (fixed per Op, or built
    // from the kernel's own enumeration), but a privileged writer must still
    // refuse anything outside /sys so a bug can't turn into an arbitrary write.
    if !target.is_absolute() || !target.starts_with("/sys/") {
        return Err(HelperError::Write {
            path: target.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "refusing to write a non-/sys path",
            ),
        });
    }
    // Skip when the attribute already holds this exact value (int or string), so
    // we don't churn the EC rewriting an unchanged setting.
    let already = std::fs::read_to_string(&target)
        .ok()
        .is_some_and(|s| s.trim() == payload);
    if !already {
        std::fs::write(&target, &payload).map_err(|source| HelperError::Write {
            path: target.display().to_string(),
            source,
        })?;
        tracing::info!("wrote {payload} to {}", target.display());
    }
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
            let p = myasus_core::kbd_backlight_path(Path::new(myasus_core::LEDS_ROOT))
                .unwrap_or_else(|| PathBuf::from(myasus_core::KBD_BACKLIGHT_PATH));
            Ok((p, v.to_string()))
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

/// The hottest thermal zone in Celsius, scanning every `thermal_zone*`. `None`
/// when none are readable.
fn thermal_max_celsius() -> Option<f64> {
    let mut max: Option<f64> = None;
    for entry in std::fs::read_dir("/sys/class/thermal").ok()?.flatten() {
        let p = entry.path();
        if !p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("thermal_zone"))
        {
            continue;
        }
        if let Ok(milli) = std::fs::read_to_string(p.join("temp")) {
            if let Ok(v) = milli.trim().parse::<f64>() {
                let c = v / 1000.0;
                max = Some(max.map_or(c, |m| m.max(c)));
            }
        }
    }
    max
}

/// The active profile as a canonical value (0=balanced, 1=performance,
/// 2=quiet) from whichever interface is live. `None` if neither is readable.
fn current_profile_raw() -> Option<u8> {
    if let Ok(s) = std::fs::read_to_string(myasus_core::FAN_PROFILE_PATH) {
        return s.trim().parse().ok();
    }
    match std::fs::read_to_string(myasus_core::PLATFORM_PROFILE_PATH)
        .ok()?
        .trim()
    {
        "performance" => Some(1),
        "balanced" => Some(0),
        "quiet" | "low-power" => Some(2),
        _ => None,
    }
}

/// Headless thermal protection: poll the sensors and force maximum cooling when
/// any zone crosses the limit, even with no GUI open. The write is transient
/// (never persisted) and never escalates privilege -- the daemon acts on its own.
/// asusctl deliberately leaves this to the EC; we add it as a safety net.
pub fn spawn_thermal_guard() {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tick.tick().await;
            let decision = tokio::task::spawn_blocking(|| {
                let max = thermal_max_celsius()?;
                let cur = current_profile_raw()?;
                myasus_core::thermal_override(max, cur).map(|p| (max, p))
            })
            .await;
            if let Ok(Some((max, profile))) = decision {
                tracing::warn!("thermal guard: {max:.0}C over limit, forcing performance");
                match tokio::task::spawn_blocking(move || perform_write(Op::FanProfile(profile))).await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::warn!("thermal guard could not force performance: {e}"),
                    Err(e) => tracing::warn!("thermal guard write task panicked: {e}"),
                }
            }
        }
    });
}

/// Record an op into the persisted state. Best-effort: a failed persist must
/// never fail the privileged write the user just authorised.
fn persist_op(op: Op) {
    let mut state = load_state();
    state.set(op);
    if let Err(e) = save_state(state) {
        tracing::warn!("could not persist state: {e}");
    }
}

fn load_state() -> DaemonState {
    std::fs::read_to_string(STATE_FILE)
        .map(|raw| DaemonState::parse(&raw))
        .unwrap_or_default()
}

fn save_state(state: DaemonState) -> std::io::Result<()> {
    atomic_write(Path::new(STATE_FILE), &state.serialize())
}

/// Atomic write: temp + rename, so a crash mid-write can't leave a half-written
/// file. Parent dir is created if missing. Path is injectable for tests.
fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

/// Re-apply every persisted setting: at daemon startup (before any GUI) and
/// after resume from suspend, since the EC forgets the charge limit across sleep
/// on many laptops. Each op is validated and written independently; one failure
/// is logged and never aborts the rest.
pub fn restore_state() {
    for op in load_state().ops() {
        if op.validate().is_err() {
            tracing::warn!("ignoring out-of-range persisted value: {op:?}");
            continue;
        }
        match perform_write(op) {
            Ok(()) => tracing::info!("restored {op:?}"),
            Err(e) => tracing::warn!("could not restore {op:?}: {e}"),
        }
    }
}

/// Subscribe to logind's `PrepareForSleep` and re-apply all persisted settings
/// when the machine resumes (`start == false`). The EC drops the charge limit
/// across suspend on many laptops, so without this our limit silently stops
/// applying after the first sleep. Best-effort: if logind is unreachable we log
/// and skip rather than fail startup.
pub async fn spawn_resume_listener(connection: &zbus::Connection) {
    let proxy = match Login1ManagerProxy::new(connection).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("cannot reach logind for resume handling: {e}");
            return;
        }
    };
    let mut stream = match proxy.receive_prepare_for_sleep().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("cannot watch PrepareForSleep: {e}");
            return;
        }
    };
    tokio::spawn(async move {
        use futures_util::stream::StreamExt as _;
        while let Some(signal) = stream.next().await {
            match signal.args() {
                // start == false means we are resuming, not going to sleep.
                Ok(args) if !args.start => {
                    tracing::info!("resumed from sleep; re-applying persisted settings");
                    // restore_state does blocking sysfs writes; keep them off the
                    // async executor.
                    tokio::task::spawn_blocking(restore_state);
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("malformed PrepareForSleep signal: {e}"),
            }
        }
    });
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
    fn atomic_write_round_trips_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/state");
        let state = DaemonState {
            charge_threshold: Some(80),
            fan_profile: Some(1),
            kbd_backlight: None,
        };
        atomic_write(&path, &state.serialize()).unwrap();
        let back = DaemonState::parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(back, state);
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn atomic_write_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state");
        atomic_write(&path, "version=1\ncharge_threshold=60\n").unwrap();
        atomic_write(&path, "version=1\ncharge_threshold=85\n").unwrap();
        let back = DaemonState::parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(back.charge_threshold, Some(85));
        assert!(!path.with_extension("tmp").exists());
    }
}
