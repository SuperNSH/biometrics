//! Linux: polkit answers whether authentication can run, fprintd reports enrolled fingers over D-Bus,
//! and robius shows the polkit prompt.
//!
//! polkit does not say whether a running authentication agent will answer. A prompt with no agent
//! fails with `ERR_BIOMETRICS_NOT_AVAILABLE` when it runs, not here.

use zbus::{Connection, zvariant::OwnedObjectPath};
use zbus_polkit::policykit1::AuthorityProxy;

use super::robius::{off_engine, start_prompt};
use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::BiometricsError,
    options::{AuthOptions, BiometricType},
};

pub(crate) struct NativeBackend;

/// fprintd's manager: its default device is the reader the user enrolled fingers on.
#[zbus::proxy(
    interface = "net.reactivated.Fprint.Manager",
    default_service = "net.reactivated.Fprint",
    default_path = "/net/reactivated/Fprint/Manager"
)]
trait FprintManager {
    fn get_default_device(&self) -> zbus::Result<OwnedObjectPath>;
}

/// One fprintd device, with the fingers enrolled for a user.
#[zbus::proxy(
    interface = "net.reactivated.Fprint.Device",
    default_service = "net.reactivated.Fprint"
)]
trait FprintDevice {
    fn list_enrolled_fingers(&self, username: &str) -> zbus::Result<Vec<String>>;
}

/// Whether polkit's authority answers on the system bus.
fn polkit_available() -> bool {
    zbus::block_on(async {
        let Ok(connection) = Connection::system().await else {
            return false;
        };
        let Ok(authority) = AuthorityProxy::new(&connection).await else {
            return false;
        };
        authority.backend_name().await.is_ok()
    })
}

/// Whether the user has a finger enrolled with fprintd. No fprintd, or no reader, is not an error.
fn fingerprint_enrolled() -> bool {
    let Ok(user) = std::env::var("USER") else {
        return false;
    };
    zbus::block_on(async {
        let Ok(connection) = Connection::system().await else {
            return false;
        };
        let Ok(manager) = FprintManagerProxy::new(&connection).await else {
            return false;
        };
        let Ok(device) = manager.get_default_device().await else {
            return false;
        };
        let Ok(device) = FprintDeviceProxy::builder(&connection)
            .path(device.as_str().to_owned())
            .build()
            .await
        else {
            return false;
        };
        device
            .list_enrolled_fingers(&user)
            .await
            .is_ok_and(|fingers| !fingers.is_empty())
    })
}

impl Backend for NativeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(off_engine(polkit_available))
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(off_engine(fingerprint_enrolled))
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        Box::pin(async {
            let enrolled = off_engine(fingerprint_enrolled).await?;
            let types = if enrolled {
                vec![BiometricType::Fingerprint]
            } else {
                Vec::new()
            };
            Ok::<_, BiometricsError>(types)
        })
    }

    fn start_prompt(
        &self,
        reason: &str,
        options: &AuthOptions,
        done: PromptDone,
    ) -> Result<(), BiometricsError> {
        start_prompt(reason, options, true, done)
    }

    fn cancel_prompt(&self) {}
}
