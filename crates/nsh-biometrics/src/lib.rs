//! Biometric and device-credential authentication for Native Shell apps.
//!
//! [`LocalAuth`] follows Flutter's `local_auth`: check what the device can do, then show a prompt
//! and wait for its answer. Each call resolves exactly once, with `Ok(true)`, `Ok(false)` or a
//! [`BiometricsError`] with a stable code.

mod auth;
mod availability;
mod backend;
mod error;
mod fake;
mod native;
mod options;
mod platform;

use nsh_core::prelude::*;

pub use auth::LocalAuth;
pub use availability::{
    Availability, AvailabilityState, BiometricAvailability, use_biometric_availability,
    use_biometric_availability_for,
};
pub use backend::{Backend, BackendFuture, PromptDone};
pub use error::{BiometricsError, ErrorKind};
pub use fake::FakeBackend;
pub use native::{NativeAuthOptions, NativeLocalAuth};
pub use options::{AuthOptions, BiometricStrength, BiometricType};

/// The crate's UI set: what TSX imports from `nsh-biometrics/native`.
#[native_module("nsh-biometrics/native")]
fn native(registry: &mut Registry) {
    registry.class::<NativeLocalAuth>();
}
