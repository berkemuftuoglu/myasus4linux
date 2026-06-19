use std::fs;
use std::path::Path;
use std::process::Command;

use super::detect;
use super::error::BackendError;

/// The only sysfs files the privileged writer may ever touch. This keeps
/// `pkexec tee` from being a write-anything-as-root primitive: even if a caller
/// is ever wrong about the path, an off-list target is refused before we
/// escalate. (The proper fix is a D-Bus helper that owns these paths itself;
/// until then this is the guard.)
fn is_allowed_target(path: &str) -> bool {
    path == detect::CHARGE_CONTROL_END_THRESHOLD
        || path == detect::THROTTLE_THERMAL_POLICY
        || path == detect::KBD_BACKLIGHT
        || is_backlight_brightness(path)
}

/// Screen backlight lives under a device-named dir, so it can't be a single
/// constant; allow exactly `/sys/class/backlight/<dev>/brightness`.
fn is_backlight_brightness(path: &str) -> bool {
    path.starts_with("/sys/class/backlight/")
        && path.ends_with("/brightness")
        && !path.contains("..")
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// Trims trailing whitespace (sysfs files typically end with a newline).
pub fn read(path: &str) -> Result<String, BackendError> {
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .map_err(|source| BackendError::SysfsRead {
            path: path.to_owned(),
            source,
        })
}

/// Read a sysfs file and parse the value as the given type.
pub fn read_value<T>(path: &str) -> Result<T, BackendError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let raw = read(path)?;
    raw.parse::<T>().map_err(|e| BackendError::ParseError {
        path: path.to_owned(),
        details: e.to_string(),
    })
}

/// Write a value directly to a sysfs file (requires appropriate permissions).
pub fn write(path: &str, value: &str) -> Result<(), BackendError> {
    fs::write(path, value).map_err(|source| BackendError::SysfsWrite {
        path: path.to_owned(),
        source,
    })
}

/// Write a value to a sysfs file using pkexec for privilege escalation.
///
/// This spawns `pkexec tee <path>` and writes the value to its stdin.
/// The user will be prompted for authentication via polkit.
pub fn write_privileged(path: &str, value: &str) -> Result<(), BackendError> {
    if !is_allowed_target(path) {
        return Err(BackendError::DisallowedPath(path.to_owned()));
    }
    let output = Command::new("pkexec")
        .args(["tee", path])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(value.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(BackendError::PrivilegedWrite)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(BackendError::PrivilegedWrite(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_exists_returns_false_for_missing() {
        assert!(!exists("/sys/nonexistent/path/does/not/exist"));
    }

    #[test]
    fn test_read_and_read_value() {
        let mut tmp = NamedTempFile::new().expect("failed to create tempfile");
        writeln!(tmp, "42").expect("failed to write");

        let path = tmp.path().to_str().expect("non-utf8 path");
        assert_eq!(read(path).expect("read failed"), "42");

        let val: u32 = read_value(path).expect("read_value failed");
        assert_eq!(val, 42);
    }

    #[test]
    fn allowlist_accepts_known_controls() {
        assert!(is_allowed_target(detect::CHARGE_CONTROL_END_THRESHOLD));
        assert!(is_allowed_target(detect::THROTTLE_THERMAL_POLICY));
        assert!(is_allowed_target(detect::KBD_BACKLIGHT));
        assert!(is_allowed_target(
            "/sys/class/backlight/intel_backlight/brightness"
        ));
        assert!(is_allowed_target(
            "/sys/class/backlight/amdgpu_bl0/brightness"
        ));
    }

    #[test]
    fn allowlist_rejects_everything_else() {
        assert!(!is_allowed_target("/etc/passwd"));
        assert!(!is_allowed_target(
            "/sys/class/backlight/../../etc/shadow/brightness"
        ));
        assert!(!is_allowed_target(
            "/sys/class/backlight/dev/max_brightness"
        ));
        assert!(!is_allowed_target("/sys/class/power_supply/BAT0/capacity"));
        assert!(!is_allowed_target(""));
    }

    #[test]
    fn privileged_write_refuses_disallowed_path() {
        let err = write_privileged("/etc/passwd", "x").unwrap_err();
        assert!(matches!(err, BackendError::DisallowedPath(_)));
    }

    #[test]
    fn test_write_roundtrip() {
        let tmp = NamedTempFile::new().expect("failed to create tempfile");
        let path = tmp.path().to_str().expect("non-utf8 path");

        write(path, "99").expect("write failed");
        assert_eq!(read(path).expect("read failed"), "99");
    }
}
