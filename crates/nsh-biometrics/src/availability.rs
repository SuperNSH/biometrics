//! The reactive availability read: the answer is a signal that wakes its bindings only when it changes.

use std::{cell::Cell, rc::Rc};

use nsh_core::platform::Lifecycle;
use nsh_core::prelude::*;

use crate::{auth::LocalAuth, error::BiometricsError, options::BiometricType};

/// What the device can do for authentication right now.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Availability {
    /// A secure lock screen or biometrics exist.
    pub device_supported: bool,
    /// Biometric hardware exists and a biometric is enrolled.
    pub can_check_biometrics: bool,
    /// The kinds of biometric the device can check.
    pub biometrics: Vec<BiometricType>,
}

/// Where the availability read stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvailabilityState {
    /// The first check has not answered yet.
    Checking,
    /// The last check answered.
    Ready(Availability),
    /// The last check failed. The error says why.
    Failed(BiometricsError),
}

/// A reactive handle on [`Availability`]. A `Copy` handle: bindings may hold it freely.
///
/// It checks when its owner starts, and again each time the app returns to the foreground, because
/// enrollment can change in the system settings. A binding on [`BiometricAvailability::state`]
/// runs again only when the answer changes.
#[derive(Clone, Copy)]
pub struct BiometricAvailability {
    state: Signal<AvailabilityState>,
    refresh: Stored<Rc<dyn Fn()>>,
}

impl BiometricAvailability {
    /// The current state. Reading it in a binding subscribes to changes.
    pub fn state(&self) -> AvailabilityState {
        self.state.get()
    }

    /// Checks again now, without waiting for the app to return to the foreground.
    pub fn refresh(&self) {
        let check = self.refresh.get();
        check();
    }
}

/// The platform's availability, read reactively in the current owner. See [`BiometricAvailability`].
pub fn use_biometric_availability() -> BiometricAvailability {
    use_biometric_availability_for(LocalAuth::new())
}

/// [`use_biometric_availability`] for a given `LocalAuth`, such as one built on a fake backend.
pub fn use_biometric_availability_for(auth: LocalAuth) -> BiometricAvailability {
    let owner = Owner::current();
    let state = signal(AvailabilityState::Checking);
    // Dropping a task cancels it, so a new check replaces a check still in flight.
    let inflight = stored(None::<Spawned<()>>);
    // The handle can outlive its owner, and a check then has no owner to land in.
    let gone = Rc::new(Cell::new(false));
    let check: Rc<dyn Fn()> = {
        let gone = Rc::clone(&gone);
        Rc::new(move || {
            if gone.get() {
                return;
            }
            let auth = auth.clone();
            let task = owner.run(|| {
                spawn(async move {
                    let answer = match read(&auth).await {
                        Ok(availability) => AvailabilityState::Ready(availability),
                        Err(error) => AvailabilityState::Failed(error),
                    };
                    state.set(answer);
                })
            });
            drop(inflight.replace(Some(task)));
        })
    };
    let on_foreground = check.clone();
    effect(move || {
        if lifecycle() == Lifecycle::Active {
            on_foreground();
        }
    });
    cleanup({
        let gone = Rc::clone(&gone);
        move || {
            gone.set(true);
            drop(inflight.replace(None));
        }
    });
    BiometricAvailability {
        state,
        refresh: stored(check),
    }
}

async fn read(auth: &LocalAuth) -> Result<Availability, BiometricsError> {
    Ok(Availability {
        device_supported: auth.is_device_supported().await?,
        can_check_biometrics: auth.can_check_biometrics().await?,
        biometrics: auth.available_biometrics().await?,
    })
}
