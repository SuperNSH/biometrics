//! Apple (iOS and macOS): `LAContext` answers availability, and robius shows the prompt.

use objc2_local_authentication::{LABiometryType, LAContext, LAPolicy};

use super::robius::{off_engine, start_prompt};
use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::BiometricsError,
    options::{AuthOptions, BiometricType},
};

pub(crate) struct NativeBackend;

/// Whether `policy` can run now. A refusal names its reason in the error, which is not a failure of the query.
fn can_evaluate(policy: LAPolicy) -> bool {
    // An LAContext is single-use, so each query makes its own.
    let context = unsafe { LAContext::new() };
    unsafe { context.canEvaluatePolicy_error(policy) }.is_ok()
}

/// The biometry the device has, once `canEvaluatePolicy` has run on this context.
fn biometry() -> LABiometryType {
    let context = unsafe { LAContext::new() };
    let _ = unsafe {
        context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
    };
    unsafe { context.biometryType() }
}

impl Backend for NativeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(off_engine(|| {
            can_evaluate(LAPolicy::DeviceOwnerAuthentication)
        }))
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(off_engine(|| {
            can_evaluate(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
        }))
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        Box::pin(async {
            let kind = off_engine(biometry).await?;
            let types = if kind == LABiometryType::FaceID {
                vec![BiometricType::Face]
            } else if kind == LABiometryType::TouchID {
                vec![BiometricType::Fingerprint]
            } else if kind == LABiometryType::OpticID {
                vec![BiometricType::Iris]
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
        // A biometric-only prompt has no passcode fallback.
        start_prompt(reason, options, !options.biometric_only, done)
    }

    fn cancel_prompt(&self) {}
}
