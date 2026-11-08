//! `LocalAuth`: the calls an app makes, with one prompt at a time and one answer per call.

use std::{
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
    time::Duration,
};

use tokio::sync::oneshot;

use crate::{
    backend::{Backend, PromptDone},
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricType},
};

type Answer = Result<bool, BiometricsError>;

/// The prompt gate. `busy` lasts until the platform answers, even after the call was cancelled,
/// so a second prompt never appears on top of one the system still shows.
///
/// `waiting` names the generation that owns the gate, so an answer, a cancel or a drop of an
/// older generation cannot touch a newer call that took over after a timeout freed the gate.
#[derive(Default)]
struct Gate {
    busy: bool,
    next_id: u64,
    waiting: Option<(u64, oneshot::Sender<Answer>)>,
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
    /// With `AuthOptions::timeout` set, the call gives up at the deadline with
    /// `ERR_BIOMETRICS_TIMEOUT`, and the platform prompt may stay until the system answers it.
    pub async fn authenticate(
        &self,
        reason: &str,
        options: &AuthOptions,
    ) -> Result<bool, BiometricsError> {
        if reason.trim().is_empty() || options.timeout == Some(Duration::ZERO) {
            return Err(BiometricsError::new(ErrorKind::InvalidArgument));
        }
        let (sender, receiver) = oneshot::channel();
        let generation = {
            let mut gate = self.shared.gate();
            if gate.busy {
                return Err(BiometricsError::new(ErrorKind::InProgress));
            }
            gate.busy = true;
            let generation = gate.next_id;
            gate.next_id += 1;
            gate.waiting = Some((generation, sender));
            generation
        };
        let shared = Arc::clone(&self.shared);
        let done: PromptDone = Box::new(move |answer| answered(&shared, generation, answer));
        if let Err(error) = self.shared.backend.start_prompt(reason, options, done) {
            let mut gate = self.shared.gate();
            gate.busy = false;
            gate.waiting = None;
            return Err(error);
        }
        let mut owner = Owner {
            shared: Arc::clone(&self.shared),
            generation,
            armed: true,
        };
        // The timer needs tokio's time driver, which wasm32 has no runtime for; there the option
        // is accepted and unused, as the web has no prompt that could time out.
        #[cfg(not(target_arch = "wasm32"))]
        let answer = match options.timeout {
            Some(timeout) => {
                tokio::select! {
                    answer = receiver => answer,
                    _ = tokio::time::sleep(timeout) => {
                        owner.armed = false;
                        free_after_timeout(&self.shared, generation);
                        Ok(Err(BiometricsError::new(ErrorKind::Timeout)))
                    }
                }
            }
            None => receiver.await,
        };
        #[cfg(target_arch = "wasm32")]
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
        if let Some((_, sender)) = waiting {
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

/// Hands the platform's answer to the waiting call, and frees the gate. An answer for a generation
/// the gate no longer holds is dropped, without touching the busy of a newer call.
fn answered(shared: &Shared, generation: u64, answer: Answer) {
    let waiting = {
        let mut gate = shared.gate();
        if matches!(&gate.waiting, Some((id, _)) if *id == generation) {
            let (_, sender) = gate.waiting.take().unwrap();
            gate.busy = false;
            Some(sender)
        } else if gate.waiting.is_none() && gate.busy {
            // The sender went to stop or a dropped call; the system's answer still ends the prompt.
            gate.busy = false;
            None
        } else {
            None
        }
    };
    if let Some(sender) = waiting {
        let _ = sender.send(answer);
    }
}

/// Frees the gate for a timed-out call whose prompt the system still shows; a second prompt may
/// stack over the first, which is why the timeout is opt-in.
#[cfg(not(target_arch = "wasm32"))]
fn free_after_timeout(shared: &Shared, generation: u64) {
    let mut gate = shared.gate();
    if matches!(&gate.waiting, Some((id, _)) if *id == generation) {
        gate.waiting = None;
        gate.busy = false;
    }
}

/// Cancels the prompt if the call's future is dropped before the answer arrives.
struct Owner {
    shared: Arc<Shared>,
    generation: u64,
    armed: bool,
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Take only the owner's own generation: after a timeout freed the gate, a newer call's
        // sender must survive the stale owner's drop.
        let taken = self
            .shared
            .gate()
            .waiting
            .take_if(|(id, _)| *id == self.generation);
        if taken.is_some() {
            self.shared.backend.cancel_prompt();
        }
    }
}
