use std::collections::HashMap;

use myasus_core::Op;
use zbus::message::Header;
use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

const ACTION_CHARGE: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetChargeThreshold";
const ACTION_FAN: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetPerformanceProfile";
const ACTION_KBD: &str = "io.github.berkmuftuoglu.MyAsus4Linux.Helper.SetKeyboardBacklight";

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

impl Helper {
    pub fn new() -> Self {
        Self
    }
}

#[zbus::interface(name = "io.github.berkmuftuoglu.MyAsus4Linux.Helper")]
impl Helper {
    async fn set_charge_threshold(
        &self,
        value: u8,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        Ok(apply(connection, &header, ACTION_CHARGE, Op::ChargeThreshold(value)).await?)
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
        Ok(apply(connection, &header, ACTION_KBD, Op::KeyboardBacklight(value)).await?)
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
    std::fs::write(path, op.raw_value().to_string()).map_err(|source| HelperError::Write {
        path,
        source,
    })?;
    tracing::info!("wrote {} to {path}", op.raw_value());
    Ok(())
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
