//! The authentication as TSX sees it: `NativeLocalAuth`, a class whose async members reject with a
//! `code` on failure. Results are plain values; the prompt and the device stay on the Rust side.

use nsh_core::prelude::*;

use crate::{AuthOptions, BiometricStrength, BiometricType, BiometricsError, ErrorKind, LocalAuth};

/// The options of one authentication, from TSX. Unset fields keep the defaults.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeAuthOptions {
    /// Only biometrics may unlock, with no device passcode fallback.
    pub biometric_only: Option<bool>,
    /// Declared as a sensitive transaction. Accepted; the prompt is the same.
    pub sensitive_transaction: Option<bool>,
    /// Keep the prompt across a pause. Accepted; no platform resumes it.
    pub sticky_auth: Option<bool>,
    /// The Android prompt's title. Defaults to the reason.
    pub title: Option<String>,
    /// The Android prompt's subtitle.
    pub subtitle: Option<String>,
    /// `"strong"` or `"weak"`. Android only.
    pub biometric_strength: Option<String>,
    /// The polkit action to check on Linux. Required there.
    pub polkit_action_id: Option<String>,
}

/// An authentication handle. Create one and keep it; every handle shares the platform's prompt gate.
#[class]
pub struct NativeLocalAuth {
    auth: LocalAuth,
}

#[class]
impl NativeLocalAuth {
    /// The platform's authentication.
    pub fn new() -> NativeLocalAuth {
        NativeLocalAuth {
            auth: LocalAuth::new(),
        }
    }

    /// Whether a secure lock screen or biometrics exist on the device.
    pub async fn is_device_supported(this: Instance<Self>) -> Result<bool, JsError> {
        let auth = this.with(|native| native.auth.clone());
        auth.is_device_supported().await.map_err(into_js)
    }

    /// Whether biometric hardware exists and a biometric is enrolled.
    pub async fn can_check_biometrics(this: Instance<Self>) -> Result<bool, JsError> {
        let auth = this.with(|native| native.auth.clone());
        auth.can_check_biometrics().await.map_err(into_js)
    }

    /// The kinds of biometric the device can check: `"face"`, `"fingerprint"`, `"iris"`, `"strong"` or `"weak"`.
    pub async fn available_biometrics(this: Instance<Self>) -> Result<Vec<String>, JsError> {
        let auth = this.with(|native| native.auth.clone());
        let kinds = auth.available_biometrics().await.map_err(into_js)?;
        Ok(kinds.into_iter().map(biometric_name).collect())
    }

    /// Shows the prompt with `reason`. Resolves `true` when the user passed, `false` when they failed.
    pub async fn authenticate(
        this: Instance<Self>,
        reason: String,
        options: Option<NativeAuthOptions>,
    ) -> Result<bool, JsError> {
        let auth = this.with(|native| native.auth.clone());
        let options = to_auth_options(options.unwrap_or_default())?;
        auth.authenticate(&reason, &options).await.map_err(into_js)
    }

    /// Cancels the running prompt. Resolves `true` when a prompt was running.
    pub async fn stop_authentication(this: Instance<Self>) -> Result<bool, JsError> {
        let auth = this.with(|native| native.auth.clone());
        auth.stop_authentication().await.map_err(into_js)
    }
}

impl Default for NativeLocalAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// The error a rejection carries: its message, and the `code` apps match on.
fn into_js(error: BiometricsError) -> JsError {
    JsError::Error(error.kind().message().to_owned()).with_code(error.code())
}

fn invalid(message: &str) -> JsError {
    JsError::Error(message.to_owned()).with_code(ErrorKind::InvalidArgument.code())
}

fn biometric_name(kind: BiometricType) -> String {
    match kind {
        BiometricType::Face => "face",
        BiometricType::Fingerprint => "fingerprint",
        BiometricType::Iris => "iris",
        BiometricType::Strong => "strong",
        BiometricType::Weak => "weak",
    }
    .to_owned()
}

/// Turns the TSX options into the crate's, refusing a strength it does not know.
fn to_auth_options(options: NativeAuthOptions) -> Result<AuthOptions, JsError> {
    let biometric_strength = match options.biometric_strength.as_deref() {
        None => None,
        Some("strong") => Some(BiometricStrength::Strong),
        Some("weak") => Some(BiometricStrength::Weak),
        Some(_) => return Err(invalid("biometricStrength must be \"strong\" or \"weak\"")),
    };
    Ok(AuthOptions {
        biometric_only: options.biometric_only.unwrap_or(false),
        sensitive_transaction: options.sensitive_transaction.unwrap_or(false),
        sticky_auth: options.sticky_auth.unwrap_or(false),
        title: options.title,
        subtitle: options.subtitle,
        biometric_strength,
        polkit_action_id: options.polkit_action_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_the_name_tsx_uses() {
        assert_eq!(biometric_name(BiometricType::Face), "face");
        assert_eq!(biometric_name(BiometricType::Fingerprint), "fingerprint");
        assert_eq!(biometric_name(BiometricType::Iris), "iris");
        assert_eq!(biometric_name(BiometricType::Strong), "strong");
        assert_eq!(biometric_name(BiometricType::Weak), "weak");
    }

    #[test]
    fn an_unknown_strength_is_refused_and_a_known_one_is_kept() {
        let refused = NativeAuthOptions {
            biometric_strength: Some("medium".to_owned()),
            ..NativeAuthOptions::default()
        };
        assert!(to_auth_options(refused).is_err());

        let weak = NativeAuthOptions {
            biometric_strength: Some("weak".to_owned()),
            ..NativeAuthOptions::default()
        };
        let options = to_auth_options(weak).ok();
        assert_eq!(
            options.and_then(|o| o.biometric_strength),
            Some(BiometricStrength::Weak)
        );
    }

    #[test]
    fn unset_options_keep_the_defaults() {
        let options = to_auth_options(NativeAuthOptions::default()).ok();
        assert_eq!(options, Some(AuthOptions::default()));
    }
}
