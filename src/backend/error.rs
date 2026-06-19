use thiserror::Error;

/// Errors that can occur when interacting with hardware via sysfs.
#[derive(Debug, Error)]
pub enum BackendError {
    #[error("failed to read sysfs attribute {path}")]
    SysfsRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write sysfs attribute {path}")]
    SysfsWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse value from {path}: {details}")]
    ParseError { path: String, details: String },

    #[error("unknown fan profile value {0}")]
    UnknownFanProfile(u8),

    #[error(transparent)]
    Validate(#[from] myasus_core::ValidateError),

    #[error("privileged daemon call failed: {0}")]
    Daemon(#[source] zbus::Error),
}
