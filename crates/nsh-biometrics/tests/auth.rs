//! `LocalAuth` through the fake backend: one prompt at a time, one answer per call, cancellation.

use std::future::Future;

use nsh_biometrics::{AuthOptions, BiometricsError, ErrorKind, FakeBackend, LocalAuth};

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// Yields until `ready` holds, so a spawned call has reached its prompt.
async fn settle(ready: impl Fn() -> bool) {
    for _ in 0..1000 {
        if ready() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the prompt never started");
}

/// Starts an authentication on its own task, and waits until its prompt is up.
async fn start_prompt(
    auth: &LocalAuth,
    fake: &FakeBackend,
) -> tokio::task::JoinHandle<Result<bool, BiometricsError>> {
    let call = auth.clone();
    let handle = tokio::spawn(async move {
        call.authenticate("Unlock your notes", &AuthOptions::default())
            .await
    });
    settle(|| fake.prompt_pending()).await;
    handle
}

const ALL_KINDS: [(ErrorKind, &str); 10] = [
    (ErrorKind::NotAvailable, "ERR_BIOMETRICS_NOT_AVAILABLE"),
    (ErrorKind::NotEnrolled, "ERR_BIOMETRICS_NOT_ENROLLED"),
    (ErrorKind::LockedOut, "ERR_BIOMETRICS_LOCKED_OUT"),
    (
        ErrorKind::PermanentlyLockedOut,
        "ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT",
    ),
    (ErrorKind::PasscodeNotSet, "ERR_BIOMETRICS_PASSCODE_NOT_SET"),
    (ErrorKind::Cancelled, "ERR_BIOMETRICS_CANCELLED"),
    (ErrorKind::InProgress, "ERR_BIOMETRICS_IN_PROGRESS"),
    (ErrorKind::NoActivity, "ERR_BIOMETRICS_NO_ACTIVITY"),
    (ErrorKind::Backend, "ERR_BIOMETRICS_BACKEND"),
    (
        ErrorKind::InvalidArgument,
        "ERR_BIOMETRICS_INVALID_ARGUMENT",
    ),
];

#[test]
fn every_kind_has_its_code_and_a_message() {
    for (kind, code) in ALL_KINDS {
        let error = BiometricsError::new(kind);
        assert_eq!(error.code(), code);
        assert!(!error.to_string().is_empty());
        assert!(!kind.message().is_empty());
    }
}

#[test]
fn every_kind_reaches_the_caller_with_its_code() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        for (kind, code) in ALL_KINDS {
            let call = start_prompt(&auth, &fake).await;
            fake.finish_prompt(Err(BiometricsError::new(kind)));
            let error = call.await.unwrap().unwrap_err();
            assert_eq!(error.code(), code);
        }
    });
}

#[test]
fn a_platform_status_is_kept_with_its_error() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let call = start_prompt(&auth, &fake).await;
        fake.finish_prompt(Err(BiometricsError::new(ErrorKind::Backend).with_status(-7)));
        let error = call.await.unwrap().unwrap_err();
        assert_eq!(error.status(), Some(-7));
    });
}

#[test]
fn a_second_prompt_is_refused_while_one_runs() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let first = start_prompt(&auth, &fake).await;
        let second = auth
            .authenticate("Again", &AuthOptions::default())
            .await
            .unwrap_err();
        assert_eq!(second.kind(), ErrorKind::InProgress);
        assert_eq!(fake.prompts_started(), 1);
        fake.finish_prompt(Ok(true));
        assert_eq!(first.await.unwrap(), Ok(true));
    });
}

#[test]
fn a_call_resolves_once_with_the_system_answer() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let passed = start_prompt(&auth, &fake).await;
        assert!(fake.finish_prompt(Ok(true)));
        assert_eq!(passed.await.unwrap(), Ok(true));
        // A second answer to a finished prompt has nothing to resolve.
        assert!(!fake.finish_prompt(Ok(false)));

        let declined = start_prompt(&auth, &fake).await;
        fake.finish_prompt(Ok(false));
        assert_eq!(declined.await.unwrap(), Ok(false));
    });
}

#[test]
fn stop_resolves_the_call_as_cancelled_and_the_late_answer_is_ignored() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let call = start_prompt(&auth, &fake).await;
        assert!(auth.stop_authentication().await.unwrap());
        let error = call.await.unwrap().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Cancelled);
        assert_eq!(fake.cancels(), 1);

        // The system still shows the prompt until it answers, so a new call waits for that answer.
        let refused = auth
            .authenticate("Again", &AuthOptions::default())
            .await
            .unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InProgress);
        fake.finish_prompt(Ok(true));

        let next = start_prompt(&auth, &fake).await;
        fake.finish_prompt(Ok(true));
        assert_eq!(next.await.unwrap(), Ok(true));
    });
}

#[test]
fn stop_with_no_prompt_running_reports_false() {
    let auth = LocalAuth::with_backend(FakeBackend::new());
    run(async {
        assert!(!auth.stop_authentication().await.unwrap());
    });
}

#[test]
fn dropping_the_call_cancels_the_prompt() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let call = start_prompt(&auth, &fake).await;
        call.abort();
        let _ = call.await;
        assert_eq!(fake.cancels(), 1, "the owner going cancels the prompt");

        let refused = auth
            .authenticate("Again", &AuthOptions::default())
            .await
            .unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InProgress);
        fake.finish_prompt(Err(BiometricsError::new(ErrorKind::Cancelled)));

        let next = start_prompt(&auth, &fake).await;
        fake.finish_prompt(Ok(true));
        assert_eq!(next.await.unwrap(), Ok(true));
    });
}

#[test]
fn a_prompt_that_fails_to_start_frees_the_gate() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        fake.fail_next_prompt(BiometricsError::new(ErrorKind::NotEnrolled));
        let error = auth
            .authenticate("Unlock", &AuthOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotEnrolled);
        assert_eq!(fake.prompts_started(), 0);

        let next = start_prompt(&auth, &fake).await;
        fake.finish_prompt(Ok(true));
        assert_eq!(next.await.unwrap(), Ok(true));
    });
}

#[test]
fn an_empty_reason_is_refused_without_a_prompt() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let error = auth
            .authenticate("   ", &AuthOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(fake.prompts_started(), 0);
    });
}

// `AuthOptions` is non-exhaustive, so a test outside the crate sets its fields one by one.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn the_reason_and_the_polkit_action_reach_the_backend() {
    let fake = FakeBackend::new();
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        let mut options = AuthOptions::default();
        options.polkit_action_id = Some("com.example.unlock".to_owned());
        let call = {
            let auth = auth.clone();
            tokio::spawn(async move { auth.authenticate("Open the vault", &options).await })
        };
        settle(|| fake.prompt_pending()).await;
        assert_eq!(fake.last_reason().as_deref(), Some("Open the vault"));
        assert_eq!(
            fake.last_polkit_action().as_deref(),
            Some("com.example.unlock")
        );
        fake.finish_prompt(Ok(true));
        assert_eq!(call.await.unwrap(), Ok(true));
    });
}

#[test]
fn the_device_queries_pass_through_the_backend() {
    let fake = FakeBackend::new();
    fake.set_can_check_biometrics(Ok(false));
    let auth = LocalAuth::with_backend(fake.clone());
    run(async {
        assert!(auth.is_device_supported().await.unwrap());
        assert!(!auth.can_check_biometrics().await.unwrap());
        assert_eq!(
            auth.available_biometrics().await.unwrap(),
            vec![nsh_biometrics::BiometricType::Face]
        );
        fake.set_device_supported(Err(BiometricsError::new(ErrorKind::Backend)));
        let error = auth.is_device_supported().await.unwrap_err();
        assert_eq!(error.code(), "ERR_BIOMETRICS_BACKEND");
    });
}
