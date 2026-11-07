# Brief: nsh-biometrics

You are building `nsh-biometrics` in `~/fo/nsh-biometrics`, remote `SuperNSH/biometrics` (public), branch `master`.

## The owner's rules

1. **Nothing builds on this machine.** No `cargo build`, `cargo test`, `cargo check`, `nsh`, Docker, emulator or simulator here, because it drains the owner's battery. GitHub Actions does all compiling and testing. Edit, commit, `git push`, then `gh run watch <id> -R SuperNSH/biometrics --interval 60 --exit-status` and read failures with `gh api repos/SuperNSH/biometrics/actions/jobs/<job id>/logs`. `cargo fmt` and `rustfmt` are fine, because they only format.
2. **Commit small and push.** After each small piece, `git add <your paths>`, commit with a short conventional message and **no attribution lines of any kind**, then `git pull --rebase` and `git push`.
3. **Use the newest stable versions** of every direct dependency, checked on crates.io when you add it. Measured on 2026-11-07: `robius-authentication` 0.3.1, `objc2-local-authentication` 0.3.2, `windows` 0.62.2, `jni` 0.22.4, `ndk-context` 0.1.1, `zbus` 6.2.1, `zbus_polkit` 6.0.0, `thiserror` 2.0.21. NSH crates are `0.4.0`.
4. Never invent an API. Read the source of each crate you use: crates.io sources are in `~/.cargo/registry/src` once fetched, or download the `.crate` file into the scratchpad and read it.
5. Do not stop to ask. Decide, write the decision in `docs/decisions.md`, and continue.

## What exists already, so reuse it

Researched on 2026-11-07:

- **NSH has nothing for this**, in 0.4.0 or on `main`.
- **`robius-authentication` 0.3.1** (Project Robius, Rust) prompts on Apple (LocalAuthentication: Face ID, Touch ID, password), Android (`android.hardware.biometrics.BiometricPrompt`, API 28+, with a Java callback it compiles to dex in `build.rs` and loads with `InMemoryDexClassLoader`), Windows (Windows Hello, with a fallback), and Linux (polkit, which its README calls incomplete). It reaches Android through `robius-android-env`, which reads `ndk-context`. **Use it for the prompt.** Do not rewrite what it does.
- What it lacks, which you write: an availability query (is it supported, is anything enrolled, which kinds), NSH's Android set-up, NSH-style reactive state, TSX support, coded errors in our scheme, and fakes for tests.
- Flutter's `local_auth` is the API to follow: `canCheckBiometrics`, `isDeviceSupported`, `getAvailableBiometrics`, `authenticate({ localizedReason, options: { biometricOnly, sensitiveTransaction, stickyAuth, useErrorDialogs } })`, `stopAuthentication`. Read its Dart API on pub.dev or its repo, `flutter/packages`, `packages/local_auth`, for behavior.
- `tauri-plugin-biometry` 0.2.8 also covers Android, iOS, macOS and Windows. Read it for how it checks availability on each platform.

Read the NSH skill first: `~/fo/nsh-starter/.agents/skills/nsh/SKILL.md`, `references/mixed.md`, `references/rust.md`, and the `coding-guides.md` sections "State ownership", "Async work and side effects", "Public API design", "Common failure modes". The NSH source is at `~/fo/.refs/nsh-main` (read only). The sibling library `~/fo/nsh-secure-storage` shows our layout, error codes, `#[native_module]` and CI, so read `crates/nsh-secure-storage/src/{lib,error,native}.rs` and `.github/workflows/ci.yml`.

## The API

Rust, in snake_case, with the same meaning as `local_auth`:

```rust
let auth = LocalAuth::new();
auth.is_device_supported().await?;          // a secure lock screen or biometrics exist
auth.can_check_biometrics().await?;         // biometric hardware exists
auth.available_biometrics().await?;         // Vec<BiometricType>: Face, Fingerprint, Iris, Strong, Weak
auth.authenticate("Unlock your notes", &AuthOptions::default()).await?; // Ok(true) passed, Ok(false) declined
auth.stop_authentication().await?;
```

- `AuthOptions` is `#[non_exhaustive]` with `Default`: `biometric_only`, `sensitive_transaction`, `sticky_auth`, plus per-platform options such as Android's `title`, `subtitle`, `negative_button`, `BiometricStrength`, and Apple's `fallback_title`, `cancel_title`, and Linux's polkit `action_id`.
- `BiometricsError` has `code()`: `ERR_BIOMETRICS_NOT_AVAILABLE`, `_NOT_ENROLLED`, `_LOCKED_OUT`, `_PERMANENTLY_LOCKED_OUT`, `_PASSCODE_NOT_SET`, `_CANCELLED`, `_IN_PROGRESS`, `_NO_ACTIVITY`, `_BACKEND`, and the platform status where there is one.
- **One prompt at a time.** A second `authenticate` while one runs gets `_IN_PROGRESS`. Each call resolves exactly once, including when its owner goes, which cancels the prompt.
- `use_biometric_availability()`: a reactive read that re-checks when the app returns to the foreground, because enrollment can change in Settings. A binding on it wakes only when the answer changes.
- `#[native_module("nsh-biometrics/native")]` exposes all of it to TSX, with Promise results that reject with a `code`.
- A `Backend` trait with a fake, so the rules above are tested on every runner without a sensor.

## Platforms

| Platform | Prompt | Availability | Notes |
|---|---|---|---|
| iOS, macOS | `robius-authentication` | `LAContext.canEvaluatePolicy` and `biometryType`, through `objc2-local-authentication` | `NSFaceIDUsageDescription` is the app's, so the docs say how to add it with NSH |
| Android | `robius-authentication` | `android.hardware.biometrics.BiometricManager.canAuthenticate`, through `jni` | NSH does not start `ndk-context`, so initialize it yourself: the `JavaVM` from `dlopen("libnativehelper.so")` and `JNI_GetCreatedJavaVMs`, and the current `Activity` through `android.app.ActivityThread` by reflection. `USE_BIOMETRIC` must be in the manifest, so find how an NSH app declares a permission, and document it |
| Windows | `robius-authentication` (Windows Hello) | `Windows.Security.Credentials.UI.UserConsentVerifier.CheckAvailabilityAsync`, through `windows` | |
| Linux | `robius-authentication` (polkit) | polkit and a running agent over `zbus` | Also report `fprintd` enrolled fingers over D-Bus, if present, for `available_biometrics` |
| Web | not available | `PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()` | `authenticate` returns `ERR_BIOMETRICS_NOT_AVAILABLE` on the web, with a docs note that WebAuthn is the web's way. Say so plainly |

## TypeScript

`packages/nsh-biometrics`: `import { LocalAuth } from "nsh-biometrics"`, with `local_auth`'s camelCase names, over `nsh-biometrics/native`. A memory or fake backend for tests. A test screen in `App.tsx`, and `nsh check`, `nsh build` and `nsh test` pass in CI. `examples/demo`: a small NSH app with a button that authenticates and shows the result, listing the crate in `nsh.json` under `"rust"`. It is what CI packages into the `latest` release.

## CI

`.github/workflows/ci.yml` exists. It runs formatting, Clippy and tests on Linux, macOS and Windows, cross-checks iOS, the iOS simulator, Android and wasm, runs the TypeScript package, and on `master` packages `examples/demo` for six targets and replaces the `latest` release. kache caches through GitHub's cache. Keep all of that, and make it pass. The Android cross-check and demo need `javac` and `d8` for `robius-authentication`'s `build.rs`. Set `ANDROID_HOME` and the build-tools path if it cannot find them.

## Docs

- `README.md`, replacing the placeholder: what this is, install for Rust and for Rust with TSX, a sample for each, a platform table, the crates it reuses and why, and what CI does and does not test.
- `docs/decisions.md`: each choice and why.
- `docs/parity.md`: one row per `local_auth` name and per `robius-authentication` feature, with its status and the test that covers it.

## Finish conditions

1. Every CI job on `master` passes, including the `latest` release with six demo builds. Paste `gh run view` output.
2. Tests through the fake backend cover: one prompt at a time, resolving exactly once, cancel when the owner goes, every error code's mapping, the availability read waking only on change, and re-checking on returning to the foreground.
3. Every platform's code compiles in CI for its target.
4. `docs/parity.md` has every `local_auth` name.
5. Say plainly in the README and your report that CI has no fingerprint reader or face camera, so no real prompt was shown there, and list what was run on a real device, which is nothing unless you say otherwise.
