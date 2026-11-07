//! The reactive availability read on `Headless`: it wakes only on change, and checks again on foreground.

use std::{
    sync::{
        Mutex, OnceLock, PoisonError,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    time::{Duration, SystemTime},
};

use nsh_biometrics::{
    AvailabilityState, BiometricType, BiometricsError, ErrorKind, FakeBackend, LocalAuth,
    use_biometric_availability_for,
};
use nsh_core::{platform::Lifecycle, prelude::*};
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_testing::Headless;

static RUNS: AtomicUsize = AtomicUsize::new(0);
static FAKE: OnceLock<FakeBackend> = OnceLock::new();
/// The engines share the fake and the counter, so the tests take turns.
static SERIAL: Mutex<()> = Mutex::new(());

fn fake() -> FakeBackend {
    FAKE.get_or_init(FakeBackend::new).clone()
}

#[component]
pub fn Probe() -> impl IntoElement {
    let availability = use_biometric_availability_for(LocalAuth::with_backend(fake()));
    view().child(text(move || {
        RUNS.fetch_add(1, SeqCst);
        match availability.state() {
            AvailabilityState::Checking => "checking".to_owned(),
            AvailabilityState::Ready(report) => format!(
                "ready {} {} {:?}",
                report.device_supported, report.can_check_biometrics, report.biometrics
            ),
            AvailabilityState::Failed(error) => format!("failed {}", error.code()),
        }
    }))
}

fn start() -> Result<Headless, String> {
    let config = EngineConfig {
        clock: Clock::manual(SystemTime::UNIX_EPOCH),
        ..EngineConfig::new(Root::of::<Probe>())
    };
    Headless::start(config).map_err(|e| format!("the engine did not start: {e}"))
}

fn reset() {
    let fake = fake();
    fake.set_device_supported(Ok(true));
    fake.set_can_check_biometrics(Ok(true));
    fake.set_available_biometrics(Ok(vec![BiometricType::Face]));
    RUNS.store(0, SeqCst);
}

fn leave_foreground(app: &mut Headless) -> Result<(), String> {
    app.set_environment("setLifecycle(background)", |e| {
        e.lifecycle = Lifecycle::Background;
    })?;
    app.run_for(Duration::from_millis(50))
}

fn return_to_foreground(app: &mut Headless) -> Result<(), String> {
    app.set_environment("setLifecycle(active)", |e| {
        e.lifecycle = Lifecycle::Active;
    })?;
    app.run_for(Duration::from_millis(50))
}

#[test]
fn the_answer_is_read_when_the_owner_starts() -> Result<(), String> {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    reset();
    let mut app = start()?;
    app.wait_for("ready true true [Face]")?;
    Ok(())
}

#[test]
fn a_binding_runs_again_only_when_the_answer_changes() -> Result<(), String> {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    reset();
    let mut app = start()?;
    app.wait_for("ready true true [Face]")?;
    let settled = RUNS.load(SeqCst);
    let reads = fake().support_reads();

    leave_foreground(&mut app)?;
    return_to_foreground(&mut app)?;
    assert!(fake().support_reads() > reads, "the foreground check ran");
    assert_eq!(
        RUNS.load(SeqCst),
        settled,
        "the same answer must not run the binding again"
    );
    Ok(())
}

#[test]
fn a_changed_answer_reaches_the_binding_on_return_to_the_foreground() -> Result<(), String> {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    reset();
    let mut app = start()?;
    app.wait_for("ready true true [Face]")?;

    // Enrollment changed in the system settings while the app was away.
    leave_foreground(&mut app)?;
    fake().set_device_supported(Ok(false));
    fake().set_can_check_biometrics(Ok(false));
    fake().set_available_biometrics(Ok(Vec::new()));
    return_to_foreground(&mut app)?;
    app.wait_for("ready false false []")?;
    Ok(())
}

#[test]
fn nothing_is_checked_while_the_app_is_in_the_background() -> Result<(), String> {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    reset();
    let mut app = start()?;
    app.wait_for("ready true true [Face]")?;
    leave_foreground(&mut app)?;
    let reads = fake().support_reads();
    app.run_for(Duration::from_millis(500))?;
    assert_eq!(fake().support_reads(), reads, "no check in the background");
    return_to_foreground(&mut app)?;
    assert_eq!(fake().support_reads(), reads + 1, "one check on return");
    Ok(())
}

#[test]
fn a_failed_check_is_reported_with_its_code() -> Result<(), String> {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    reset();
    fake().set_device_supported(Err(BiometricsError::new(ErrorKind::Backend)));
    let mut app = start()?;
    app.wait_for("failed ERR_BIOMETRICS_BACKEND")?;
    reset();
    Ok(())
}
