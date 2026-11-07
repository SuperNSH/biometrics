//! The errors an authentication call reports. Each has a stable code; a message says what to do.

use std::fmt;

/// What went wrong, as a stable code; see [`BiometricsError::code`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Biometrics and device credentials are not available here (no sensor, no lock screen, or the web).
    NotAvailable,
    /// No biometric is enrolled on the device.
    NotEnrolled,
    /// Too many failed attempts. Biometrics may return after a while, or after the device passcode.
    LockedOut,
    /// Biometrics are locked until the user signs in with the device passcode.
    PermanentlyLockedOut,
    /// The device has no passcode, which the prompt needs.
    PasscodeNotSet,
    /// The user, the app or the system cancelled the prompt.
    Cancelled,
    /// Another prompt is already running. One prompt runs at a time.
    InProgress,
    /// The prompt has no screen to show on, for example Android with no running app.
    NoActivity,
    /// The platform failed; `status` holds its own code where it gives one.
    Backend,
    /// An argument is empty or not allowed on this platform, such as a reason of only spaces.
    InvalidArgument,
}

/// A failed authentication call or availability query.
///
/// `Display` names the code and the message. Neither holds the reason text or any user data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .kind.message())]
pub struct BiometricsError {
    kind: ErrorKind,
    status: Option<i64>,
}

impl BiometricsError {
    /// An error of `kind`, with no platform status.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, status: None }
    }

    /// The same error, carrying the platform's own status code.
    #[must_use]
    pub fn with_status(mut self, status: i64) -> Self {
        self.status = Some(status);
        self
    }

    /// What went wrong.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The platform's status code, when the platform gave one.
    pub fn status(&self) -> Option<i64> {
        self.status
    }

    /// The stable code, such as `ERR_BIOMETRICS_CANCELLED`. Apps match on this.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl ErrorKind {
    /// The stable code of this kind.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::NotAvailable => "ERR_BIOMETRICS_NOT_AVAILABLE",
            ErrorKind::NotEnrolled => "ERR_BIOMETRICS_NOT_ENROLLED",
            ErrorKind::LockedOut => "ERR_BIOMETRICS_LOCKED_OUT",
            ErrorKind::PermanentlyLockedOut => "ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT",
            ErrorKind::PasscodeNotSet => "ERR_BIOMETRICS_PASSCODE_NOT_SET",
            ErrorKind::Cancelled => "ERR_BIOMETRICS_CANCELLED",
            ErrorKind::InProgress => "ERR_BIOMETRICS_IN_PROGRESS",
            ErrorKind::NoActivity => "ERR_BIOMETRICS_NO_ACTIVITY",
            ErrorKind::Backend => "ERR_BIOMETRICS_BACKEND",
            ErrorKind::InvalidArgument => "ERR_BIOMETRICS_INVALID_ARGUMENT",
        }
    }

    /// What to do about it, in one sentence.
    pub fn message(self) -> &'static str {
        match self {
            ErrorKind::NotAvailable => {
                "Biometrics and device credentials are not available on this device."
            }
            ErrorKind::NotEnrolled => "No biometric is enrolled. Add one in the system settings.",
            ErrorKind::LockedOut => {
                "Too many failed attempts. Try again later or use the device passcode."
            }
            ErrorKind::PermanentlyLockedOut => {
                "Biometrics are locked. Sign in with the device passcode to unlock them."
            }
            ErrorKind::PasscodeNotSet => "Set a device passcode in the system settings first.",
            ErrorKind::Cancelled => "The authentication was cancelled.",
            ErrorKind::InProgress => {
                "Another authentication is already running. Wait for it to finish."
            }
            ErrorKind::NoActivity => "There is no screen to show the prompt on right now.",
            ErrorKind::Backend => "The platform's authentication service failed.",
            ErrorKind::InvalidArgument => "An argument is empty or not allowed on this platform.",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
