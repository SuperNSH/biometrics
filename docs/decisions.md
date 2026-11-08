# Decisions

Each entry says what was chosen and why. `docs/parity.md` lists what each API does on each platform.

## The prompt comes from robius-authentication, on every platform

The brief says to use `robius-authentication` for the prompt and not to rewrite it. Its 0.3.1 API has a callback and no cancel. So the library calls it on Apple, Android, Windows and Linux, and writes only what robius lacks: availability, the gate, the error codes and the NSH surface.

Consequence: `stop_authentication` and dropping a call cannot dismiss the system prompt. The call resolves at once with `ERR_BIOMETRICS_CANCELLED`, and the gate stays closed until the system answers, so a second prompt never appears on top of the first. A call with a `timeout` is the exception, and the option is off by default (see below). The alternative, calling `LAContext` ourselves on Apple, would make Apple cancellable and leave Android and Windows not. That is a bigger change than the brief asks for, so it waits for a reason.

## One gate per process, and an answer frees it

`LocalAuth::new()` returns one shared handle per process. `with_backend` gives a handle its own gate, for tests and fakes. The gate is busy from the start of a call until the platform answers, not until the call is resolved. A late answer after a cancel is dropped, because the call has already resolved. A `timeout` call ends that busy period early; the next entry says what that trades.

A call dropped before its answer cancels the native prompt (`cancel_prompt`). That is the closest the library can get, given the previous decision.

## A timeout is opt-in, and it frees the gate early

`timeout` on `AuthOptions` races the platform answer against a tokio timer. When the timer wins, the call resolves `ERR_BIOMETRICS_TIMEOUT` and the gate is force-freed, so the app can call again at once. A generation guard drops the platform's answer to the timed-out call, whenever it arrives after that.

The trade: the system prompt stays on screen, so the next call puts a new prompt on top of the still-visible first one. That is why `timeout` is off by default.

The timer needs tokio's `time` feature, which the library now enables. On `wasm32` the timer is compiled out, because tokio has no timer driver there. The web has no prompt to time out, and the TypeScript layer runs its own watchdog for `timeout`, forwarding `timeoutMs` to the native options so the Rust gate frees on the same deadline.

A `timeout` of zero is `ERR_BIOMETRICS_INVALID_ARGUMENT`, refused before the gate is claimed, like an empty reason.

## `Ok(false)` is an answer, and a cancel is an error

`Ok(false)` covers a failed match or a declined prompt, since the user chose that outcome. A user cancel, a system cancel and an app cancel are `ERR_BIOMETRICS_CANCELLED`. This is the split `local_auth` makes, with the cancel as a code instead of an exception, as the brief's error list asks.

## Error codes

The brief's ten codes are in `error.rs`, with two additions: `ERR_BIOMETRICS_INVALID_ARGUMENT`, for an empty reason and for a Linux call with no polkit action, and `ERR_BIOMETRICS_TIMEOUT`, for the opt-in `timeout`. Without the first, those cases would have to be reported as `ERR_BIOMETRICS_BACKEND`, which says something false about the platform.

`ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT` is in the enum and the fake backend returns it. robius-authentication 0.3.1 maps Android's permanent lockout to `Exhausted`, and the library cannot tell the two apart without its own Android callback. The row is marked Partial in `parity.md`.

Platform status codes are kept where robius gives one. robius 0.3.1 gives none on Apple, so Apple's `LAError` code is not in the error.

## robius remaps: `UserFallback` and `Timeout`

robius-authentication reports `UserFallback` when the user taps the passcode-fallback button and backs out, which happens on Apple's biometric-only policy. That is a user cancel, so it maps to `ERR_BIOMETRICS_CANCELLED`, not `ERR_BIOMETRICS_BACKEND`.

robius's `Timeout` maps to `ERR_BIOMETRICS_TIMEOUT`, the code the opt-in `timeout` option resolves with.

## Availability is a signal that wakes only on change

`use_biometric_availability()` holds one `Signal<AvailabilityState>`. The signal compares by equality, so a read that gives the same answer does not run a binding again. The read starts when the owner starts and again each time `lifecycle()` becomes `Active`, because enrollment can change in the system settings while the app is away. A new check drops an older one still in flight. The owner going drops the task, and a `refresh` on the handle that escaped it does nothing, on either side of the library.

## The web read treats a missing `lifecycle` as active

NSH 0.4.0's web runtime ships no `lifecycle` accessor: the engine declares it under `nsh/device`, but the runtime replaces that namespace with a shorter one, so a TS import of `lifecycle` from `"nsh"` stays `undefined` for the app's whole run. The headless engine has it, which is why `nsh test` passes while the packaged web app crashed at mount. The TypeScript read treats a missing `lifecycle` as `"active"` — the first check still runs; the return-to-foreground re-check is what waits for an NSH release that provides the accessor. The Rust read is unaffected: it reads the engine's own lifecycle, which the web build has.

## Apple availability through LAContext, not robius

robius has no availability query. The library asks `LAContext.canEvaluatePolicy` for the two policies it needs (device owner, and biometrics only) and reads `biometryType`. Each query uses a fresh `LAContext`, since the brief's sources say a used one can skip the prompt.

## Apple availability gates on enrollment

`available_biometrics` reports a kind only when a biometric is enrolled. The biometrics-only `canEvaluatePolicy` query is the gate: with nothing enrolled, it fails and the answer is empty. This matches `local_auth`.

## Android: the application context, not the Activity

The brief says to find the current Activity through `ActivityThread` by reflection. The library uses `ActivityThread.currentApplication()` instead. It is a public static method, so it needs no reflection on hidden fields. The framework's `BiometricPrompt` and `BiometricManager` take any `Context`, and the prompt is a system dialog, so it does not need an Activity. If the runtime has not created the application yet, the call reports `ERR_BIOMETRICS_NO_ACTIVITY`.

NSH does not start `ndk-context`, so `ensure_context` finds the VM through `libnativehelper.so` and `JNI_GetCreatedJavaVMs`, as `nsh-secure-storage` does, and installs the context once. `ndk-context` asserts a second initialize, so `ensure_context` adopts an existing context or initializes one (see below).

The prompt start runs on the engine thread, unlike the availability queries, which the blocking pool answers: the `Backend` trait's `start_prompt` is synchronous by design, and robius's Android prompt does all its JNI work in that call. The first prompt of a process also loads robius's dex class, so it can drop frames. Warming the class is not possible from this library; robius keeps it private.

### ndk-context is adopted when another crate installed one

`ndk_context::initialize_android_context` asserts when a context already exists, and another library in the process can have installed one. `ensure_context` therefore probes `ndk_context::android_context()` inside `catch_unwind`. A probe that returns means a context exists, and the library adopts it. A probe that panics means none exists, and the library initializes one. A mutex serializes the probe and the initialize. The probe panics once on a fresh process, and a silent panic hook keeps that out of the log; a build with `panic = "abort"` cannot use the probe and aborts there instead.

### USE_BIOMETRIC is not declared, and this library cannot declare it

`android.permission.USE_BIOMETRIC` must be in the app's manifest. NSH 0.4.0 writes the manifest itself. It declares `INTERNET`, and `POST_NOTIFICATIONS` when the app has the `notifications` capability, and no setting adds other permissions (`crates/nsh-cli/src/android.rs`). So an NSH app cannot get the permission from `nsh.json` today. Until NSH offers a way, a prompt on Android fails with the framework's `SecurityException`, which robius reports as `ERR_BIOMETRICS_BACKEND`. This is a gap in NSH, and the README says so.

## Windows: the blocking call uses windows-future's `join`

`UserConsentVerifier::CheckAvailabilityAsync` returns an `IAsyncOperation`. Waiting for it on a blocking-pool thread needs `join()`, which `windows-future` 0.3.2 provides only with its `std` feature. The `windows` crate does not turn that feature on, so the library enables it. The version matches the one `windows` 0.62.2 resolves.

Windows Hello reports one answer for the device, so `available_biometrics` reports `strong` when it is available and nothing otherwise. The prompt always allows the PIN, which Hello requires, so `biometric_only` has no effect on Windows.

## Windows: COM initialization is balanced

The Windows calls need COM initialized on the thread that runs them. The library initializes the Windows Runtime with `RoInitialize`, and a guard calls `RoUninitialize` on drop, so the thread is left as it found it. When `RoInitialize` answers `RPC_E_CHANGED_MODE`, another library initialized the thread in a different apartment, and the release is skipped: this library did not do the initialize.

## Linux: zbus 6 for the queries, robius for the prompt

robius-authentication 0.3.1 uses zbus 5 for the prompt. The library's availability queries use zbus 6.2.1 and zbus_polkit 6.0.0, the newest versions. Both versions can live in one build, so the prompt keeps robius's own.

- `is_device_supported` is true when polkit's authority answers on the system bus. polkit does not say whether an authentication agent will answer, so a prompt with no agent fails when it runs, with `ERR_BIOMETRICS_NOT_AVAILABLE`.
- `can_check_biometrics` and `available_biometrics` ask fprintd (`net.reactivated.Fprint`) for the fingers enrolled by `$USER` on the default device. No fprintd is not an error; it is no biometric.
- `authenticate` needs `polkit_action_id`, which names an action in a policy file the app installs. The library has no default action, since a default would prompt for an unrelated privilege.

## An empty polkit action is refused like a missing one

`polkit_action_id` that is empty or only whitespace gets `ERR_BIOMETRICS_INVALID_ARGUMENT`, like a missing one. Neither names an action for polkit to check.

## Web: reported as unsupported

The web has no API that verifies the user: WebAuthn is a ceremony bound to an origin and a registered credential, not a prompt a library can call, and the platform-authenticator check only says whether WebAuthn could run, not whether this library can do anything. The first release answered availability with that check, which set a trap — `can_check_biometrics` was `true` while `authenticate` always failed — so the web now reports what the library can do: nothing. Every availability answer is false, `authenticate` returns `ERR_BIOMETRICS_NOT_AVAILABLE`, and the web module is the same unsupported backend any other target without a prompt gets. An app that wants WebAuthn calls it directly; the check is one line and the ceremony is the app's anyway.

The `wasm-bindgen`, `wasm-bindgen-futures` and `js-sys` dependencies are gone with the check.

## iOS needs an Info.plist key, and NSH 0.4.0 cannot set it

Apple requires `NSFaceIDUsageDescription` for Face ID. The brief asks the docs to say how to add it with NSH. NSH's main branch takes `ios.infoPlist` in `nsh.json`, but the released 0.4.0 CLI, which CI installs, rejects an `ios` key with `unknown field ios`. The demo therefore has no such key. The README says so, and the key goes in once an NSH release accepts it. A library cannot write an app's `Info.plist` itself.

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

## The web demo deploys to GitHub Pages

NSH's web build is static with relative asset paths, so it serves from the `/biometrics/` subpath a project page gets. CI deploys the `demo-web` artifact, the same tarball the `latest` release takes, rather than building the demo twice. Pages is set to be served by the workflow (`build_type: workflow`) in the repo settings once, because `configure-pages` v6 no longer turns Pages on with `GITHUB_TOKEN`.

## Versions

Each direct dependency is the newest stable release on crates.io on 2026-11-07, as `docs/brief.md` asks, with two pins that a newer release would break:

- `windows-future` is 0.3.2, the version `windows` 0.62.2 resolves. A different version would give a different `IAsyncOperation` type.
- `libc` is 0.2.191. Its 1.0 release is an alpha.

The web once pinned `wasm-bindgen` to `=0.2.129`, the release `nsh` 0.4.0 runs; the pins left with the WebAuthn check the web module no longer makes.
