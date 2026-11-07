//! The seam between `LocalAuth` and a platform: the queries, and one prompt that answers once.

use std::{future::Future, pin::Pin};

use crate::{
    error::BiometricsError,
    options::{AuthOptions, BiometricType},
};

/// A future a backend returns. Each call's future borrows the backend.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Receives the system's answer to one prompt, exactly once: `Ok(true)` passed, `Ok(false)` declined.
pub type PromptDone = Box<dyn FnOnce(Result<bool, BiometricsError>) + Send>;

/// What a platform provides. [`crate::LocalAuth`] adds the rules: one prompt at a time, one answer per call.
pub trait Backend: Send + Sync {
    /// Whether a secure lock screen or biometrics exist on the device.
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>>;

    /// Whether biometric hardware exists and has a biometric enrolled.
    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>>;

    /// The kinds of biometric the device can check now.
    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>>;

    /// Shows the prompt and returns at once. `done` is called once, when the system answers.
    /// An `Err` here means no prompt was shown, and `done` is not called.
    fn start_prompt(
        &self,
        reason: &str,
        options: &AuthOptions,
        done: PromptDone,
    ) -> Result<(), BiometricsError>;

    /// Asks the system to dismiss the prompt. Where the platform cannot dismiss it, this does nothing,
    /// and the prompt's answer still arrives through `done`.
    fn cancel_prompt(&self);
}
