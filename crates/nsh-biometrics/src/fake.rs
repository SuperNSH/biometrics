//! A backend with scripted answers. Tests use it, and so can an app that runs without a sensor.

use std::{
    future::ready,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::BiometricsError,
    options::{AuthOptions, BiometricType},
};

/// A backend whose answers the test sets, and whose prompt the test finishes.
///
/// Like the platform prompts, a cancel does not finish the prompt: the test answers it, as the
/// system would. Clones share one state.
#[derive(Clone)]
pub struct FakeBackend {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    device_supported: Result<bool, BiometricsError>,
    can_check: Result<bool, BiometricsError>,
    biometrics: Result<Vec<BiometricType>, BiometricsError>,
    next_start_error: Option<BiometricsError>,
    waiting: Option<PromptDone>,
    prompts: usize,
    cancels: usize,
    support_reads: usize,
    last_reason: Option<String>,
    last_polkit_action: Option<String>,
}

impl Default for FakeBackend {
    /// A device with a lock screen and one face, enrolled.
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    /// A device with a lock screen and an enrolled face.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                device_supported: Ok(true),
                can_check: Ok(true),
                biometrics: Ok(vec![BiometricType::Face]),
                next_start_error: None,
                waiting: None,
                prompts: 0,
                cancels: 0,
                support_reads: 0,
                last_reason: None,
                last_polkit_action: None,
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets what `is_device_supported` answers.
    pub fn set_device_supported(&self, answer: Result<bool, BiometricsError>) {
        self.lock().device_supported = answer;
    }

    /// Sets what `can_check_biometrics` answers.
    pub fn set_can_check_biometrics(&self, answer: Result<bool, BiometricsError>) {
        self.lock().can_check = answer;
    }

    /// Sets what `available_biometrics` answers.
    pub fn set_available_biometrics(&self, answer: Result<Vec<BiometricType>, BiometricsError>) {
        self.lock().biometrics = answer;
    }

    /// Makes the next prompt fail to start with `error`, as a platform does when it cannot show one.
    pub fn fail_next_prompt(&self, error: BiometricsError) {
        self.lock().next_start_error = Some(error);
    }

    /// Whether a prompt is waiting for its answer.
    pub fn prompt_pending(&self) -> bool {
        self.lock().waiting.is_some()
    }

    /// Answers the waiting prompt. Returns `false` when none waits.
    pub fn finish_prompt(&self, answer: Result<bool, BiometricsError>) -> bool {
        let waiting = self.lock().waiting.take();
        match waiting {
            Some(done) => {
                done(answer);
                true
            }
            None => false,
        }
    }

    /// How many prompts were started.
    pub fn prompts_started(&self) -> usize {
        self.lock().prompts
    }

    /// How many times a prompt was asked to cancel.
    pub fn cancels(&self) -> usize {
        self.lock().cancels
    }

    /// How many times the device support was read.
    pub fn support_reads(&self) -> usize {
        self.lock().support_reads
    }

    /// The reason the last prompt showed.
    pub fn last_reason(&self) -> Option<String> {
        self.lock().last_reason.clone()
    }

    /// The polkit action the last prompt named.
    pub fn last_polkit_action(&self) -> Option<String> {
        self.lock().last_polkit_action.clone()
    }
}

impl Backend for FakeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        let answer = {
            let mut state = self.lock();
            state.support_reads += 1;
            state.device_supported.clone()
        };
        Box::pin(ready(answer))
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(ready(self.lock().can_check.clone()))
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        Box::pin(ready(self.lock().biometrics.clone()))
    }

    fn start_prompt(
        &self,
        reason: &str,
        options: &AuthOptions,
        done: PromptDone,
    ) -> Result<(), BiometricsError> {
        let mut state = self.lock();
        state.last_reason = Some(reason.to_owned());
        state.last_polkit_action = options.polkit_action_id.clone();
        if let Some(error) = state.next_start_error.take() {
            return Err(error);
        }
        state.prompts += 1;
        state.waiting = Some(done);
        Ok(())
    }

    fn cancel_prompt(&self) {
        self.lock().cancels += 1;
    }
}
