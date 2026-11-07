# Decisions

Each entry says what was chosen and why. `docs/parity.md` lists what each API does on each platform.

## The prompt comes from robius-authentication, on every platform

The brief says to use `robius-authentication` for the prompt and not to rewrite it. Its 0.3.1 API has a callback and no cancel. So the library calls it on Apple, Android, Windows and Linux, and writes only what robius lacks: availability, the gate, the error codes and the NSH surface.

Consequence: `stop_authentication` and dropping a call cannot dismiss the system prompt. The call resolves at once with `ERR_BIOMETRICS_CANCELLED`, and the gate stays closed until the system answers, so a second prompt never appears on top of the first. The alternative, calling `LAContext` ourselves on Apple, would make Apple cancellable and leave Android and Windows not. That is a bigger change than the brief asks for, so it waits for a reason.

## One gate per process, and an answer frees it

`LocalAuth::new()` returns one shared handle per process. `with_backend` gives a handle its own gate, for tests and fakes. The gate is busy from the start of a call until the platform answers, not until the call is resolved. A late answer after a cancel is dropped, because the call has already resolved.

A call dropped before its answer cancels the native prompt (`cancel_prompt`). That is the closest the library can get, given the previous decision.

## `Ok(false)` is an answer, and a cancel is an error

`Ok(false)` covers a failed match or a declined prompt, since the user chose that outcome. A user cancel, a system cancel and an app cancel are `ERR_BIOMETRICS_CANCELLED`. This is the split `local_auth` makes, with the cancel as a code instead of an exception, as the brief's error list asks.

## Error codes

The brief's ten codes are in `error.rs`, with one addition: `ERR_BIOMETRICS_INVALID_ARGUMENT`, for an empty reason and for a Linux call with no polkit action. Without it, those cases would have to be reported as `ERR_BIOMETRICS_BACKEND`, which says something false about the platform.

`ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT` is in the enum and the fake backend returns it. robius-authentication 0.3.1 maps Android's permanent lockout to `Exhausted`, and the library cannot tell the two apart without its own Android callback. The row is marked Partial in `parity.md`.

Platform status codes are kept where robius gives one. robius 0.3.1 gives none on Apple, so Apple's `LAError` code is not in the error.

## Availability is a signal that wakes only on change

`use_biometric_availability()` holds one `Signal<AvailabilityState>`. The signal compares by equality, so a read that gives the same answer does not run a binding again. The read starts when the owner starts and again each time `lifecycle()` becomes `Active`, because enrollment can change in the system settings while the app is away. A new check drops an older one still in flight. The owner going drops the task.

## Apple availability through LAContext, not robius

robius has no availability query. The library asks `LAContext.canEvaluatePolicy` for the two policies it needs (device owner, and biometrics only) and reads `biometryType`. Each query uses a fresh `LAContext`, since the brief's sources say a used one can skip the prompt.

## Android: the application context, not the Activity

The brief says to find the current Activity through `ActivityThread` by reflection. The library uses `ActivityThread.currentApplication()` instead. It is a public static method, so it needs no reflection on hidden fields. The framework's `BiometricPrompt` and `BiometricManager` take any `Context`, and the prompt is a system dialog, so it does not need an Activity. If the runtime has not created the application yet, the call reports `ERR_BIOMETRICS_NO_ACTIVITY`.

NSH does not start `ndk-context`, so `ensure_context` finds the VM through `libnativehelper.so` and `JNI_GetCreatedJavaVMs`, as `nsh-secure-storage` does, and calls `ndk_context::initialize_android_context` once. `ndk-context` asserts a second call, so a mutex guards the first.

### USE_BIOMETRIC is not declared, and this library cannot declare it

`android.permission.USE_BIOMETRIC` must be in the app's manifest. NSH 0.4.0 writes the manifest itself. It declares `INTERNET`, and `POST_NOTIFICATIONS` when the app has the `notifications` capability, and no setting adds other permissions (`crates/nsh-cli/src/android.rs`). So an NSH app cannot get the permission from `nsh.json` today. Until NSH offers a way, a prompt on Android fails with the framework's `SecurityException`, which robius reports as `ERR_BIOMETRICS_BACKEND`. This is a gap in NSH, and the README says so.

## Windows: the blocking call uses windows-future's `join`

`UserConsentVerifier::CheckAvailabilityAsync` returns an `IAsyncOperation`. Waiting for it on a blocking-pool thread needs `join()`, which `windows-future` 0.3.2 provides only with its `std` feature. The `windows` crate does not turn that feature on, so the library enables it. The version matches the one `windows` 0.62.2 resolves.

Windows Hello reports one answer for the device, so `available_biometrics` reports `strong` when it is available and nothing otherwise. The prompt always allows the PIN, which Hello requires, so `biometric_only` has no effect on Windows.

## Linux: zbus 6 for the queries, robius for the prompt

robius-authentication 0.3.1 uses zbus 5 for the prompt. The library's availability queries use zbus 6.2.1 and zbus_polkit 6.0.0, the newest versions. Both versions can live in one build, so the prompt keeps robius's own.

- `is_device_supported` is true when polkit's authority answers on the system bus. polkit does not say whether an authentication agent will answer, so a prompt with no agent fails when it runs, with `ERR_BIOMETRICS_NOT_AVAILABLE`.
- `can_check_biometrics` and `available_biometrics` ask fprintd (`net.reactivated.Fprint`) for the fingers enrolled by `$USER` on the default device. No fprintd is not an error; it is no biometric.
- `authenticate` needs `polkit_action_id`, which names an action in a policy file the app installs. The library has no default action, since a default would prompt for an unrelated privilege.

## Web: the WebAuthn check, and no prompt

The web has no secret store or biometric prompt the page can call. `authenticate` returns `ERR_BIOMETRICS_NOT_AVAILABLE`. The availability read calls `PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()` through the global object, so a page without WebAuthn answers `false` and does not fail. The check uses `js-sys` reflection, not `web-sys` features, so no feature list has to match a browser API.

The wasm-bindgen version is pinned to `=0.2.129`, the one NSH 0.4.0 runs, as `nsh-secure-storage` does.

## Options a platform cannot apply are accepted

The brief lists `negative_button`, `fallback_title` and `cancel_title`, and robius-authentication takes none of them. They are accepted and passed through, and the platform shows its own text. `sensitive_transaction`, `sticky_auth` and `useErrorDialogs` are accepted for the same reason. Each is marked in `parity.md`.

## The TypeScript gate mirrors the Rust gate

`LocalAuth` in TypeScript gates calls the same way: one prompt at a time, the answer resolves the call once, and `stopAuthentication` resolves the pending call with `ERR_BIOMETRICS_CANCELLED` and leaves the gate closed until the backend answers. JavaScript has no owner that can go, so the TypeScript side has no drop-to-cancel. A component that unmounts should call `stopAuthentication`.

`useBiometricAvailability` has the same change rule as the Rust read, and `lifecycle()` drives the foreground check.

## The native module exposes a plain class

`NativeLocalAuth` is a `#[class]` whose async members reject with a `code`. Values are plain: booleans, strings and a list of kind names. The prompt and the device stay on the Rust side. `to_auth_options` refuses a strength it does not know, with `ERR_BIOMETRICS_INVALID_ARGUMENT`.

## Tests run where the sensor is not

- `tests/auth.rs` runs `LocalAuth` through `FakeBackend` on every runner, covering the gate, cancel, drop, each code and each start failure.
- `tests/availability.rs` runs the reactive read on `Headless`, with a manual clock. It covers change-only wakes, the foreground re-check, a background pause and a failed check.
- The robius mapping has unit tests in `platform/robius.rs`, which run on Linux, macOS and Windows.
- The TypeScript tests use `memoryBackend`, so the gate and the availability read run without the native crate's answers.

No test shows a real prompt. A test that opened one could hang on a runner whose account has a password. `App.tsx` and the TypeScript tests press only Stop, which needs no prompt.

## Versions

Each direct dependency is the newest stable release on crates.io on 2026-11-07, as `docs/brief.md` asks, with three pins that a newer release would break:

- `wasm-bindgen` is `=0.2.129`, the runtime `nsh` 0.4.0 runs for the web. `wasm-bindgen-futures` 0.4.79 and `js-sys` 0.3.106 are the releases that match it; the newer ones need wasm-bindgen 0.2.130.
- `windows-future` is 0.3.2, the version `windows` 0.62.2 resolves. A different version would give a different `IAsyncOperation` type.
- `libc` is 0.2.191. Its 1.0 release is an alpha.

The wasm pins are the ones `nsh-secure-storage` builds with in CI.
