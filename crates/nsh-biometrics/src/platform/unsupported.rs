//! A target with no biometric backend: nothing is available, and a prompt cannot be shown.

use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricType},
};

pub(crate) struct NativeBackend;

impl Backend for NativeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(std::future::ready(Ok::<bool, BiometricsError>(false)))
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(std::future::ready(Ok::<bool, BiometricsError>(false)))
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        Box::pin(std::future::ready(
            Ok::<Vec<BiometricType>, BiometricsError>(Vec::new()),
        ))
    }

    fn start_prompt(
        &self,
        _reason: &str,
        _options: &AuthOptions,
        _done: PromptDone,
    ) -> Result<(), BiometricsError> {
        Err(BiometricsError::new(ErrorKind::NotAvailable))
    }

    fn cancel_prompt(&self) {}
}
