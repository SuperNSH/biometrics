//! Windows: Windows Hello availability through `UserConsentVerifier`, and robius shows the prompt.
//!
//! `UserConsentVerifier` reports one answer for the device, so it does not say which biometric
//! exists. The prompt always allows the PIN, which Windows Hello requires, so `biometric_only` has no effect here.

use windows::{
    Security::Credentials::UI::{UserConsentVerifier, UserConsentVerifierAvailability},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
};

use super::robius::{off_engine, start_prompt};
use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricType},
};

pub(crate) struct NativeBackend;

/// Asks Windows Hello whether it can verify the user now. Blocks until WinRT answers.
fn availability() -> Result<UserConsentVerifierAvailability, BiometricsError> {
    // SAFETY: WinRT's initializer for this thread. An error means the thread is already set up in another mode, which is fine.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let operation = UserConsentVerifier::CheckAvailabilityAsync().map_err(windows_error)?;
    operation.join().map_err(windows_error)
}

fn windows_error(error: windows::core::Error) -> BiometricsError {
    BiometricsError::new(ErrorKind::Backend).with_status(i64::from(error.code().0))
}

fn ready(availability: UserConsentVerifierAvailability) -> bool {
    availability == UserConsentVerifierAvailability::Available
}

impl Backend for NativeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(async { off_engine(availability).await.and_then(|a| a.map(ready)) })
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(async { off_engine(availability).await.and_then(|a| a.map(ready)) })
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        Box::pin(async {
            let types = match off_engine(availability).await.and_then(|a| a.map(ready))? {
                true => vec![BiometricType::Strong],
                false => Vec::new(),
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
