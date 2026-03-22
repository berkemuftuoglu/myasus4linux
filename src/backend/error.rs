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

    #[error("invalid charge threshold {0} (must be 40-100)")]
    InvalidThreshold(u8),

    #[error("invalid keyboard brightness {0} (must be 0-3)")]
    InvalidBrightness(u8),

    #[error("unknown fan profile value {0}")]
    UnknownFanProfile(u8),

    #[error("privileged write failed")]
    PrivilegedWrite(#[source] std::io::Error),

    #[error("feature not supported on this model: {0}")]
    NotSupported(String),
}
