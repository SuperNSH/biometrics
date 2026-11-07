//! The kinds of biometric and the options of a prompt.

/// A kind of biometric or strength class, as `available_biometrics` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiometricType {
    /// A face, read by a camera (Face ID on Apple, face recognition on Android and Windows Hello).
    Face,
    /// A fingerprint (Touch ID, or a fingerprint sensor on Android and Linux).
    Fingerprint,
    /// An iris, or the Optic ID on Apple.
    Iris,
    /// Android's strong class, or Windows Hello.
    Strong,
    /// Android's weak class.
    Weak,
}

/// The Android strength a biometric prompt requires. Other platforms ignore it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BiometricStrength {
    /// Class 3 sensors only, such as a fingerprint reader that the device vouches for.
    #[default]
    Strong,
    /// Class 2 sensors too.
    Weak,
}

/// The options of one authentication. Start from [`AuthOptions::default`] and set what you need.
///
/// Options a platform cannot apply are accepted and ignored, as `docs/parity.md` lists them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthOptions {
    /// Only biometrics may unlock: no device passcode or password fallback.
    pub biometric_only: bool,
    /// Declared to the platform as a sensitive transaction. Accepted; the platform prompt is the same.
    pub sensitive_transaction: bool,
    /// Keep the prompt across a pause of the app. Accepted; no platform resumes a prompt for NSH yet.
    pub sticky_auth: bool,
    /// The Android prompt's title. Defaults to the reason.
    pub title: Option<String>,
    /// The Android prompt's subtitle.
    pub subtitle: Option<String>,
    /// The Android strength class. Defaults to strong.
    pub biometric_strength: Option<BiometricStrength>,
    /// The polkit action that Linux checks, as the policy file names it. Required on Linux.
    pub polkit_action_id: Option<String>,
}
