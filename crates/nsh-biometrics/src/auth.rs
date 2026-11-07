//! `LocalAuth`: the calls an app makes, with one prompt at a time and one answer per call.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use tokio::sync::oneshot;

use crate::{
    backend::{Backend, PromptDone},
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricType},
};

type Answer = Result<bool, BiometricsError>;

/// The prompt gate. `busy` lasts until the platform answers, even after the call was cancelled,
/// so a second prompt never appears on top of one the system still shows.
#[derive(Default)]
struct Gate {
    busy: bool,
    waiting: Option<oneshot::Sender<Answer>>,
}

struct Shared {
    backend: Arc<dyn Backend>,
    gate: Mutex<Gate>,
}

impl Shared {
    fn gate(&self) -> MutexGuard<'_, Gate> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Authentication with the device's biometrics or credentials, as `local_auth` names it.
///
/// Clones share one prompt gate. [`LocalAuth::new`] returns the platform's shared handle, so two
/// calls in one app share it too.
#[derive(Clone)]
pub struct LocalAuth {
    shared: Arc<Shared>,
}

impl LocalAuth {
    /// The platform's authentication, with one prompt gate for the whole process.
    pub fn new() -> Self {
        static PLATFORM: OnceLock<LocalAuth> = OnceLock::new();
        PLATFORM
            .get_or_init(|| Self::from_arc(crate::platform::default_backend()))
            .clone()
    }

    /// Authentication through `backend`, with its own prompt gate. Tests and fakes use this.
    pub fn with_backend(backend: impl Backend + 'static) -> Self {
        Self::from_arc(Arc::new(backend))
    }

    fn from_arc(backend: Arc<dyn Backend>) -> Self {
        Self {
            shared: Arc::new(Shared {
                backend,
                gate: Mutex::new(Gate::default()),
            }),
        }
    }

    /// Whether a secure lock screen or biometrics exist on the device.
    pub async fn is_device_supported(&self) -> Result<bool, BiometricsError> {
        self.shared.backend.is_device_supported().await
    }

    /// Whether biometric hardware exists and a biometric is enrolled.
    pub async fn can_check_biometrics(&self) -> Result<bool, BiometricsError> {
        self.shared.backend.can_check_biometrics().await
    }

    /// The kinds of biometric the device can check now.
    pub async fn available_biometrics(&self) -> Result<Vec<BiometricType>, BiometricsError> {
        self.shared.backend.available_biometrics().await
    }

    /// Shows the prompt with `reason` and waits for its answer.
    ///
    /// `Ok(true)` when the user passed, `Ok(false)` when they failed or declined. Every other
    /// outcome is an error with a code: a cancel, a lockout, no enrollment, and so on. A second call
    /// while one prompt runs gets `ERR_BIOMETRICS_IN_PROGRESS`. Dropping the future cancels the prompt.
    pub async fn authenticate(
        &self,
        reason: &str,
        options: &AuthOptions,
    ) -> Result<bool, BiometricsError> {
        if reason.trim().is_empty() {
            return Err(BiometricsError::new(ErrorKind::InvalidArgument));
        }
        let (sender, receiver) = oneshot::channel();
        {
            let mut gate = self.shared.gate();
            if gate.busy {
                return Err(BiometricsError::new(ErrorKind::InProgress));
            }
            gate.busy = true;
            gate.waiting = Some(sender);
        }
        let shared = Arc::clone(&self.shared);
        let done: PromptDone = Box::new(move |answer| answered(&shared, answer));
        if let Err(error) = self.shared.backend.start_prompt(reason, options, done) {
            let mut gate = self.shared.gate();
            gate.busy = false;
            gate.waiting = None;
            return Err(error);
        }
        let mut owner = Owner {
            shared: Arc::clone(&self.shared),
            armed: true,
        };
        let answer = receiver.await;
        owner.armed = false;
        answer.unwrap_or_else(|_| Err(BiometricsError::new(ErrorKind::Backend)))
    }

    /// Cancels the running prompt. The pending `authenticate` call resolves with
    /// `ERR_BIOMETRICS_CANCELLED`. Returns `true` when a prompt was running.
    pub async fn stop_authentication(&self) -> Result<bool, BiometricsError> {
        let (waiting, busy) = {
            let mut gate = self.shared.gate();
            (gate.waiting.take(), gate.busy)
        };
        if let Some(sender) = waiting {
            let _ = sender.send(Err(BiometricsError::new(ErrorKind::Cancelled)));
        }
        if busy {
            self.shared.backend.cancel_prompt();
        }
        Ok(busy)
    }
}

impl Default for LocalAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Hands the platform's answer to the waiting call, and frees the gate.
fn answered(shared: &Shared, answer: Answer) {
    let waiting = {
        let mut gate = shared.gate();
        gate.busy = false;
        gate.waiting.take()
    };
    if let Some(sender) = waiting {
        let _ = sender.send(answer);
    }
}

/// Cancels the prompt if the call's future is dropped before the answer arrives.
struct Owner {
    shared: Arc<Shared>,
    armed: bool,
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let waiting = self.shared.gate().waiting.take();
        if waiting.is_some() {
            self.shared.backend.cancel_prompt();
        }
    }
}
