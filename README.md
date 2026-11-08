# nsh-biometrics

Biometric and device-credential authentication for [Native Shell](https://nsh.dev) apps, in Rust and in TSX: Face ID and Touch ID, Android biometrics, Windows Hello, and the Linux desktop's own prompt through polkit. The names follow Flutter's [`local_auth`](https://pub.dev/packages/local_auth), so an app that knows `local_auth` knows this one. The native work builds on [`robius-authentication`](https://github.com/project-robius/robius).

What it does:

- Asks whether the device can authenticate at all, whether a biometric is enrolled, and which kinds exist.
- Shows the system prompt and returns `true` (passed), `false` (failed or declined), or an error with a stable code such as `ERR_BIOMETRICS_LOCKED_OUT`.
- Lets one prompt run at a time, and answers each call exactly once.
- Re-checks availability when the app returns to the foreground, since enrollment can change in the system settings.

`docs/parity.md` lists every `local_auth` name and what it does here. `docs/decisions.md` says why each choice was made.

## Install and use it in Rust

Add the crate to `Cargo.toml`:

```toml
[dependencies]
nsh-biometrics = { git = "https://github.com/SuperNSH/biometrics" }
```

```rust
use nsh_biometrics::{AuthOptions, BiometricsError, LocalAuth};

async fn unlock(auth: &LocalAuth) -> Result<bool, BiometricsError> {
    if !auth.is_device_supported().await? {
        return Ok(false);
    }
    auth.authenticate("Unlock your notes", &AuthOptions::default()).await
}
```

Match on `error.code()` for the cases an app handles, such as `ERR_BIOMETRICS_NOT_ENROLLED` or `ERR_BIOMETRICS_CANCELLED`.

For a reactive screen, read availability with `use_biometric_availability()` inside a component. A binding on its `state()` runs again only when the answer changes.

## Install and use it in TSX

List the crate under `"rust"` in the app's `nsh.json`, and add the TypeScript package from `packages/nsh-biometrics` to the app:

```json
{
  "name": "My app",
  "id": "com.example.app",
  "version": "0.1.0",
  "build": 1,
  "rust": "path/to/nsh-biometrics"
}
```

```tsx
import { mount, Text, View } from "nsh";
import { BiometricsError, LocalAuth } from "nsh-biometrics";

const auth = new LocalAuth();

function App() {
  const unlock = () => {
    auth.authenticate({ localizedReason: "Unlock your notes" }).then(
      (passed) => console.log(passed ? "unlocked" : "not unlocked"),
      (error: unknown) => console.log(error instanceof BiometricsError ? error.code : "unknown error"),
    );
  };
  return (
    <View class="flex-1 p-4">
      <Text role="button" onPress={unlock} class="rounded-(--radius) bg-muted px-3 py-2">Unlock</Text>
    </View>
  );
}

mount(App);
```

**iOS needs `NSFaceIDUsageDescription` in `Info.plist` before an app uses Face ID, and NSH 0.4.0 cannot write it.** That release rejects an `ios` key in `nsh.json`. The main branch of NSH reads `ios.infoPlist`, so a later release will take the key. Until then, Apple ends an iOS app that asks for Face ID without the key. Touch ID and the passcode do not need it.

`examples/demo` is a complete app: a button that authenticates and shows the result. Its web build runs at [supernsh.github.io/biometrics](https://supernsh.github.io/biometrics/), redeployed on every push to `master`.

## Platforms

| Platform | Prompt | Availability | Status |
|---|---|---|---|
| iOS, macOS | robius-authentication, through `LAContext` | `LAContext.canEvaluatePolicy` and `biometryType` | Builds in CI. The prompt is not shown in CI |
| Android | robius-authentication `BiometricPrompt` (API 28+) | `BiometricManager.canAuthenticate`, `KeyguardManager.isDeviceSecure` | Builds in CI. Needs `USE_BIOMETRIC` in the manifest; see below |
| Windows | robius-authentication (Windows Hello) | `UserConsentVerifier.CheckAvailabilityAsync` | Builds and tests in CI |
| Linux | robius-authentication (polkit) | polkit's authority, and fprintd's enrolled fingers over D-Bus | Builds and tests in CI. Needs a `polkit_action_id` and an installed policy file |
| Web | none: `authenticate` returns `ERR_BIOMETRICS_NOT_AVAILABLE` | Nothing: the web is an unsupported target | Builds in CI. The page has no API that verifies the user; WebAuthn is the web's way, and an app that wants it calls it directly |

Limits:

- **Cancel does not dismiss the system prompt.** robius-authentication has no cancel. `stopAuthentication` and a dropped call resolve at once with `ERR_BIOMETRICS_CANCELLED`, but the system prompt stays until the user answers it. A new call gets `ERR_BIOMETRICS_IN_PROGRESS` until then. A `timeout` is the one exception: it force-frees the gate.
- **A `timeout` frees the gate but not the screen.** `timeout` on `AuthOptions` (`timeoutMs` in TSX) resolves the call with `ERR_BIOMETRICS_TIMEOUT` when the duration passes, and the gate frees for the next call. The first prompt is still on screen, so a newer prompt can stack over it. That trade is why it is off by default.
- **Android needs the permission, and NSH cannot add it.** `android.permission.USE_BIOMETRIC` must be in the generated manifest. NSH 0.4.0 declares only `INTERNET`, plus `POST_NOTIFICATIONS` for the `notifications` capability, and no setting adds other permissions. Until NSH offers one, a prompt on Android fails with `ERR_BIOMETRICS_BACKEND`.
- **iOS Face ID needs an `Info.plist` key that NSH 0.4.0 cannot set.** See the TSX section above.
- **Some options are accepted and not applied.** `sensitiveTransaction`, `stickyAuth`, the Android negative button, and Apple's fallback and cancel titles. `docs/parity.md` lists them.
- **Permanent lockout is reported as a lockout.** `ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT` exists, but robius-authentication does not report that state separately.
- **The web is an unsupported target.** Every availability answer is `false` and `authenticate` rejects with `ERR_BIOMETRICS_NOT_AVAILABLE`. The page has no API that verifies the user: WebAuthn is a ceremony bound to an origin and a registered credential, not a prompt a library can call, so an app that wants it uses it directly.
- **Windows reports `["strong"]`, not a sensor list.** With Windows Hello available, `getAvailableBiometrics` reports `["strong"]` even on a PIN-only device: `UserConsentVerifier` gives one answer for the device and cannot say which sensors back it. An app must not render 'fingerprint or face available' from it.
- **NSH 0.4.0's web runtime has no `lifecycle` accessor.** A TS import of `lifecycle` from `"nsh"` stays `undefined` there, so `useBiometricAvailability` treats it as active: the first check runs, the foreground re-check does not. An NSH release with the accessor restores it.

## The crates it reuses, and why

- [`robius-authentication`](https://crates.io/crates/robius-authentication) 0.3.1 is the prompt on four platforms. Project Robius maintains it, and it does what the prompt needs, so the library does not reimplement it.
- [`objc2-local-authentication`](https://crates.io/crates/objc2-local-authentication) 0.3.2 makes the Apple availability calls, which robius-authentication does not expose.
- [`jni`](https://crates.io/crates/jni) 0.22.4 and [`ndk-context`](https://crates.io/crates/ndk-context) 0.1.1 make the Android availability calls over JNI, and provide the VM and application context that robius reads.
- [`windows`](https://crates.io/crates/windows) 0.62.2 makes the Windows Hello availability call.
- [`zbus`](https://crates.io/crates/zbus) 6.2.1 and [`zbus_polkit`](https://crates.io/crates/zbus_polkit) 6.0.0 make the polkit and fprintd queries over D-Bus.
- [`tokio`](https://crates.io/crates/tokio), with its `sync` and `time` features, provides the one-shot channel that resolves a call and the timer behind `timeout`.
- [`thiserror`](https://crates.io/crates/thiserror) 2.0.21 derives the error's `Display`.
- `nsh-core`, `nsh-engine` and `nsh-testing` 0.4.0 provide NSH's reactivity, `#[native_module]` and the headless test driver.

Flutter's `local_auth` set the API, and `tauri-plugin-biometry` was the reference for each platform's availability check. `nsh-secure-storage` set the layout, the error codes and the CI.

## What CI does, and what it does not

CI runs on every push to `master` and on pull requests. Every job runs on GitHub's runners.

- **Format**: `cargo fmt --check`.
- **Rust** on Linux, macOS and Windows: Clippy with `-D warnings` over all targets, then the test suite. The tests use the fake backend and NSH's headless engine.
- **Cross-check** for iOS, the iOS simulator, Android and wasm: Clippy for each target, which compiles each platform's code.
- **TypeScript**: `nsh check`, `nsh build` with no warnings, and `nsh test` for the package.
- **Demo**, on `master` only: `examples/demo` is packaged for macOS, Linux and Windows, each on both x86_64 and arm64, plus the iOS simulator, Android and the web, and the `latest` release is replaced with those builds.
- **Pages**, on `master` only: the demo's web build is deployed to GitHub Pages, from the same artifact the release takes.

CI does **not**:

- show a system prompt on any platform;
- have a fingerprint reader, a face camera, an Apple Watch, a Windows Hello sensor, or a polkit authentication agent;
- run the Android build on a device or an emulator, or the iOS build on a simulator with an enrolled biometric;
- check that a permission such as `USE_BIOMETRIC` is in an app's manifest.

**No real prompt has been shown by this code, and it has not been run on a real device.** The tests check what the library passes to the platform and how it answers to scripted results. What a sensor does is the platform's behaviour, and it has not been seen here.

## Development

Nothing builds on a developer's machine. Compile, test and package in CI, and read the results with `gh run`. `cargo fmt` is the only local command the repo expects.

## License

MIT, as in `LICENSE`.
