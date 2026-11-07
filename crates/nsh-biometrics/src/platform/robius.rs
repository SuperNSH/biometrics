//! The prompt, from `robius-authentication`, shared by the platforms that have one.
//!
//! robius answers through a callback and has no way to cancel a prompt, so a cancelled call is
//! answered by `LocalAuth` and the late answer is dropped. `docs/parity.md` lists this.

use std::sync::{Mutex, PoisonError};

use robius_authentication as robius;

use crate::{
    backend::PromptDone,
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricStrength},
};

/// Runs `work` on the blocking pool, so a platform call never waits on the engine thread.
pub(crate) async fn off_engine<R: Send + 'static>(
    work: impl FnOnce() -> R + Send + 'static,
) -> Result<R, BiometricsError> {
    nsh_core::prelude::blocking(work)
        .await
        .map_err(|_| BiometricsError::new(ErrorKind::Backend))
}

/// Shows the prompt. `done` gets the answer once; an `Err` here means no prompt was shown.
pub(crate) fn start_prompt(
    reason: &str,
    options: &AuthOptions,
    password: bool,
    done: PromptDone,
) -> Result<(), BiometricsError> {
    let policy = policy(options, password)?;
    let title = options.title.as_deref().unwrap_or(reason);
    let text = robius::Text {
        android: robius::AndroidText {
            title,
            subtitle: options.subtitle.as_deref(),
            description: None,
        },
        apple: reason,
        windows: robius::WindowsText::new_truncated(title, reason),
    };
    let done = Mutex::new(Some(done));
    let context = robius::Context::new(());
    context
        .authenticate(text, &policy, move |answer| {
            let done = done.lock().unwrap_or_else(PoisonError::into_inner).take();
            if let Some(done) = done {
                done(map_answer(answer));
            }
        })
        .map_err(|error| match map_answer(Err(error)) {
            Err(error) => error,
            Ok(_) => BiometricsError::new(ErrorKind::Backend),
        })
}

fn policy(options: &AuthOptions, password: bool) -> Result<robius::Policy, BiometricsError> {
    let strength = match options.biometric_strength.unwrap_or_default() {
        BiometricStrength::Strong => robius::BiometricStrength::Strong,
        BiometricStrength::Weak => robius::BiometricStrength::Weak,
    };
    let mut builder = robius::PolicyBuilder::new()
        .biometrics(Some(strength))
        .password(password)
        .companion(false);
    if let Some(action) = &options.polkit_action_id {
        builder = builder.action_ids([action.as_str()]);
    }
    // robius refuses a policy it cannot show: on Linux, one without a polkit action.
    builder
        .build()
        .ok_or(BiometricsError::new(ErrorKind::InvalidArgument))
}

/// The answer a prompt gives: `Ok(true)` passed, `Ok(false)` failed or was declined by the user.
pub(crate) fn map_answer(answer: robius::Result<()>) -> Result<bool, BiometricsError> {
    match answer {
        Ok(()) => Ok(true),
        Err(robius::Error::Authentication) => Ok(false),
        Err(error) => Err(map_error(&error)),
    }
}

/// The public code of a robius error. Every error but a failed match maps to one code.
pub(crate) fn map_error(error: &robius::Error) -> BiometricsError {
    use robius::Error as E;
    let kind = match error {
        // A failed match is an answer, not an error; this arm only sees a prompt that failed to start.
        E::Authentication => ErrorKind::Backend,
        E::Exhausted => ErrorKind::LockedOut,
        E::Unavailable | E::BiometryDisconnected | E::NotPaired | E::DisabledByPolicy => {
            ErrorKind::NotAvailable
        }
        E::UserCanceled | E::AppCanceled | E::SystemCanceled => ErrorKind::Cancelled,
        E::NotEnrolled | E::NotConfigured => ErrorKind::NotEnrolled,
        E::PasscodeNotSet => ErrorKind::PasscodeNotSet,
        E::InvalidActionId | E::InvalidText => ErrorKind::InvalidArgument,
        E::NotInteractive => ErrorKind::NoActivity,
        E::CompanionNotAvailable
        | E::InvalidDimensions
        | E::UserFallback
        | E::UpdateRequired
        | E::Timeout
        | E::Busy
        | E::Unknown => ErrorKind::Backend,
        #[cfg(target_os = "android")]
        E::Java(_) => ErrorKind::Backend,
    };
    BiometricsError::new(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_match_is_an_answer_and_a_pass_is_true() {
        assert_eq!(map_answer(Ok(())), Ok(true));
        assert_eq!(map_answer(Err(robius::Error::Authentication)), Ok(false));
    }

    #[test]
    fn each_robius_error_maps_to_one_code() {
        let cases = [
            (robius::Error::Exhausted, ErrorKind::LockedOut),
            (robius::Error::Unavailable, ErrorKind::NotAvailable),
            (robius::Error::BiometryDisconnected, ErrorKind::NotAvailable),
            (robius::Error::NotPaired, ErrorKind::NotAvailable),
            (robius::Error::DisabledByPolicy, ErrorKind::NotAvailable),
            (robius::Error::UserCanceled, ErrorKind::Cancelled),
            (robius::Error::AppCanceled, ErrorKind::Cancelled),
            (robius::Error::SystemCanceled, ErrorKind::Cancelled),
            (robius::Error::NotEnrolled, ErrorKind::NotEnrolled),
            (robius::Error::NotConfigured, ErrorKind::NotEnrolled),
            (robius::Error::PasscodeNotSet, ErrorKind::PasscodeNotSet),
            (robius::Error::InvalidActionId, ErrorKind::InvalidArgument),
            (robius::Error::InvalidText, ErrorKind::InvalidArgument),
            (robius::Error::NotInteractive, ErrorKind::NoActivity),
            (robius::Error::CompanionNotAvailable, ErrorKind::Backend),
            (robius::Error::InvalidDimensions, ErrorKind::Backend),
            (robius::Error::UserFallback, ErrorKind::Backend),
            (robius::Error::UpdateRequired, ErrorKind::Backend),
            (robius::Error::Timeout, ErrorKind::Backend),
            (robius::Error::Busy, ErrorKind::Backend),
            (robius::Error::Unknown, ErrorKind::Backend),
        ];
        for (error, kind) in cases {
            assert_eq!(
                map_answer(Err(error.clone())),
                Err(BiometricsError::new(kind))
            );
        }
    }

    #[test]
    fn a_policy_without_a_polkit_action_is_refused() {
        let options = AuthOptions::default();
        if cfg!(target_os = "linux") {
            assert_eq!(
                policy(&options, true).err().map(|e| e.kind()),
                Some(ErrorKind::InvalidArgument)
            );
        } else {
            assert!(policy(&options, true).is_ok());
        }
    }
}
