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

    /// The daemon's version, so the GUI can probe reachability at startup and
    /// surface a version skew instead of throwing a raw error on the first write.
    #[expect(clippy::unused_self, reason = "zbus interface methods take &self")]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
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
    // The writes are tiny sysfs + state-file writes (sub-millisecond), so do them
    // inline. We must NOT use tokio::task::spawn_blocking here: this handler runs
    // on zbus's own executor thread, which has no Tokio runtime, so spawn_blocking
    // would panic ("no reactor running").
    perform_write(op)?;
    persist_op(op);
    Ok(())
}

/// Resolve the target + payload, refuse anything that is not an absolute /sys
/// attribute, skip a redundant write, then write. Synchronous: called on a
/// blocking thread from `apply`, and directly at startup/resume from
/// [`restore_state`].
fn perform_write(op: Op) -> Result<(), HelperError> {
    // Validate here so the write primitive itself enforces the range -- every
    // caller (D-Bus handler, boot/resume restore, thermal guard) goes through it,
    // and none of them can skip the check by construction.
    op.validate()?;
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
    if !Path::new(myasus_core::PLATFORM_PROFILE_PATH).exists() {
        return Err(HelperError::NoFanControl);
    }
    // Map the canonical profile to whichever token this firmware actually offers
    // (Zenbook/Vivobook often expose `low-power` but not `quiet`). The candidate
    // list is the shared core mapping, not a third copy of it.
    let token = myasus_core::profile_tokens(value)
        .iter()
        .copied()
        .find(|t| platform_profile_supports(t))
        .ok_or(HelperError::NoFanControl)?;
    Ok((PathBuf::from(myasus_core::PLATFORM_PROFILE_PATH), token.to_owned()))
}

fn platform_profile_supports(token: &str) -> bool {
    std::fs::read_to_string(myasus_core::PLATFORM_PROFILE_CHOICES_PATH)
        .is_ok_and(|s| s.split_whitespace().any(|c| c == token))
}

/// The hottest plausible thermal zone in Celsius, via the shared core reader
/// (implausible sentinels like 128C dropped). `None` when none are readable.
fn thermal_max_celsius() -> Option<f64> {
    myasus_core::hottest_zone(Path::new(myasus_core::THERMAL_ROOT)).map(|z| z.celsius)
}

/// The active profile as a canonical value (0=balanced, 1=performance,
/// 2=quiet) from whichever interface is live. `None` if neither is readable.
fn current_profile_raw() -> Option<u8> {
    if let Ok(s) = std::fs::read_to_string(myasus_core::FAN_PROFILE_PATH) {
        return s.trim().parse().ok();
    }
    myasus_core::profile_from_token(&std::fs::read_to_string(myasus_core::PLATFORM_PROFILE_PATH).ok()?)
}

/// Headless thermal protection: poll the sensors and force maximum cooling when
/// any zone crosses the limit, even with no GUI open. The write is transient
/// (never persisted) and never escalates privilege -- the daemon acts on its own.
/// asusctl deliberately leaves this to the EC; we add it as a safety net.
pub fn spawn_thermal_guard() {
    const PERFORMANCE: u8 = 1;
    tokio::spawn(async move {
        // First tick fires immediately, so a hot boot/restart is re-evaluated at
        // once (restore_state has already re-applied the user's intended profile).
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        let mut overridden: Option<u8> = None;
        loop {
            tick.tick().await;
            let Some((max, cur)) = tokio::task::spawn_blocking(|| Some((thermal_max_celsius()?, current_profile_raw()?)))
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            let (action, next) = myasus_core::guard_step(max, cur, overridden);
            overridden = next;
            match action {
                myasus_core::ThermalAction::Force => {
                    tracing::warn!("thermal guard: {max:.0}C over limit, forcing performance (was {cur})");
                    if !write_profile(PERFORMANCE).await {
                        overridden = None; // write failed -- don't claim we own the override
                    }
                }
                myasus_core::ThermalAction::Restore(snapshot) => {
                    // Restore the user's persisted intent, falling back to what we
                    // snapshotted at override time if nothing was ever saved.
                    let target = load_state().fan_profile.unwrap_or(snapshot);
                    tracing::info!("thermal guard: cooled to {max:.0}C, restoring profile {target}");
                    write_profile(target).await;
                }
                myasus_core::ThermalAction::None => {}
            }
        }
    });
}

/// Write a fan profile from the thermal guard, off the async executor. Returns
/// whether it succeeded. Transient (not persisted) and unprivileged-by-polkit:
/// the daemon acts on its own for safety.
async fn write_profile(value: u8) -> bool {
    match tokio::task::spawn_blocking(move || perform_write(Op::FanProfile(value))).await {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            tracing::warn!("thermal guard write failed: {e}");
            false
        }
        Err(e) => {
            tracing::warn!("thermal guard write task panicked: {e}");
            false
        }
    }
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

/// Atomic + durable write: write a unique temp, fsync its contents, rename over
/// the target, then fsync the directory so the rename itself survives power loss
/// (rename only guarantees visibility atomicity, not durability) -- the exact
/// failure this persisted state exists to survive. Path is injectable for tests.
fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Unique temp so two writers can't clobber each other's scratch file.
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Re-apply every persisted setting: at daemon startup (before any GUI) and
/// after resume from suspend, since the EC forgets the charge limit across sleep
/// on many laptops. Each op is validated and written independently; one failure
/// is logged and never aborts the rest.
pub fn restore_state() {
    let state = load_state();
    for op in state.ops() {
        // perform_write validates; an out-of-range persisted value surfaces here
        // as a write error and is logged, not silently applied.
        match perform_write(op) {
            Ok(()) => tracing::info!("restored {op:?}"),
            Err(e) => tracing::warn!("could not restore {op:?}: {e}"),
        }
    }

    // Safeguard #1: on first run (nothing persisted) default the charge limit to
    // 80% when the control exists, so battery longevity is protected out of the
    // box. Persisted so it's a one-time default the user can then override.
    let has_charge_control =
        myasus_core::charge_threshold_path(Path::new(myasus_core::POWER_SUPPLY_ROOT)).is_some();
    if state.charge_threshold.is_none() && has_charge_control {
        let op = Op::ChargeThreshold(myasus_core::CHARGE_DEFAULT);
        match perform_write(op) {
            Ok(()) => {
                persist_op(op);
                tracing::info!(
                    "first run: defaulted charge limit to {}%",
                    myasus_core::CHARGE_DEFAULT
                );
            }
            Err(e) => tracing::warn!("could not apply first-run charge default: {e}"),
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
        assert!(!has_temp_leftover(&path));
    }

    #[test]
    fn atomic_write_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state");
        atomic_write(&path, "charge_threshold=60\n").unwrap();
        atomic_write(&path, "charge_threshold=85\n").unwrap();
        let back = DaemonState::parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(back.charge_threshold, Some(85));
        assert!(!has_temp_leftover(&path));
    }

    /// True if any sibling temp file from `atomic_write` was left behind.
    fn has_temp_leftover(path: &Path) -> bool {
        let parent = path.parent().unwrap();
        std::fs::read_dir(parent)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains("tmp"))
    }
}
