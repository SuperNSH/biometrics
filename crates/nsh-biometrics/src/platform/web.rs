//! The web: no prompt is shown here. The page's own WebAuthn check says whether a platform
//! authenticator can verify the user, and `authenticate` refuses with `ERR_BIOMETRICS_NOT_AVAILABLE`.
//! On the web, WebAuthn is the way to ask for a user's verification.

use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricType},
};

pub(crate) struct NativeBackend;

/// `PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()`, read through the global
/// object so a page without WebAuthn answers `false` rather than failing.
async fn platform_authenticator_available() -> bool {
    let global = js_sys::global();
    let Ok(credential) = Reflect::get(&global, &JsValue::from_str("PublicKeyCredential")) else {
        return false;
    };
    let Ok(check) = Reflect::get(
        &credential,
        &JsValue::from_str("isUserVerifyingPlatformAuthenticatorAvailable"),
    ) else {
        return false;
    };
    let Ok(check) = check.dyn_into::<Function>() else {
        return false;
    };
    let Ok(promise) = check.call0(&credential) else {
        return false;
    };
    let Ok(promise) = promise.dyn_into::<Promise>() else {
        return false;
    };
    JsFuture::from(promise)
        .await
        .ok()
        .and_then(|answer| answer.as_bool())
        .unwrap_or(false)
}

impl Backend for NativeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(async { Ok::<_, BiometricsError>(platform_authenticator_available().await) })
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(async { Ok::<_, BiometricsError>(platform_authenticator_available().await) })
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        // WebAuthn does not say which kind of verification the device has.
        Box::pin(async { Ok::<_, BiometricsError>(Vec::new()) })
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
