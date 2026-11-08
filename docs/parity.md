# Parity with `local_auth` and `robius-authentication`

Each row names one API of Flutter's `local_auth` (the names come from `packages/local_auth` in `flutter/packages`, read on 2026-11-07) or one feature of `robius-authentication` 0.3.1, the status here, and the test that covers it.

Status words:

- **Done**: works on every platform the row names, as Flutter does.
- **Partial**: works, with a limit named in the row.
- **Accepted**: the name is accepted and passed on, but the platform does nothing with it.
- **Platform only**: the effect exists on a device and no CI runner can show it; the test checks what the library passes on.

Rust test names are in `crates/nsh-biometrics/tests/`. TypeScript tests are in `packages/nsh-biometrics/tests/`.

## Methods of `LocalAuthentication`

| `local_auth` | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `authenticate({ localizedReason })` | `LocalAuth::authenticate(reason, &options)` | `LocalAuth.authenticate({ localizedReason })` | Done. `Ok(true)` passed, `Ok(false)` failed or declined, an error with a code otherwise | `a_call_resolves_once_with_the_system_answer`; "a decline resolves false" in `auth.test.tsx` |
| `authenticate(authMessages: …)` | not in the API | not in the API | Partial. Android title and subtitle come from `title` and `subtitle`. The other texts (iOS fallback and cancel, Android cancel, Windows caption) are the platform's own. robius-authentication 0.3.1 takes only these | `the_reason_and_the_polkit_action_reach_the_backend` |
| `authenticate(biometricOnly:)` | `AuthOptions::biometric_only` | `biometricOnly` | Platform only. Apple and Android drop the passcode fallback. Linux (polkit) and Windows (Hello always allows a PIN) ignore it | `options and the reason reach the backend` in `auth.test.tsx` |
| `authenticate(sensitiveTransaction:)` | `AuthOptions::sensitive_transaction` | `sensitiveTransaction` | Accepted. No platform API used here changes its prompt for it | `options and the reason reach the backend` in `auth.test.tsx` |
| `authenticate(persistAcrossBackgrounding:)` | `AuthOptions::sticky_auth` | `stickyAuth` | Accepted. No platform resumes a prompt for NSH. Flutter's `stickyAuth` maps here | none beyond the field's pass-through |
| `authenticate(options.useErrorDialogs:)` | not in the API | `useErrorDialogs` (accepted) | Accepted. Flutter always sends `false` for it, so the field is kept only for shape | none |
| not in `local_auth` | `AuthOptions::timeout` | `timeout` | Done. Opt-in. The call resolves `ERR_BIOMETRICS_TIMEOUT` when the duration passes, and the gate frees, so a newer prompt can stack over the still-visible one. The Rust timer is compiled out on wasm32, where there is no prompt to time out; the TypeScript layer keeps its own watchdog | `a_call_with_a_timeout_resolves_with_the_timeout_code`; "a timeout resolves with the code and the gate recovers" in `auth.test.tsx` |
| `stopAuthentication()` | `LocalAuth::stop_authentication()` | `LocalAuth.stopAuthentication()` | Partial. The pending call resolves with `ERR_BIOMETRICS_CANCELLED` at once. robius-authentication has no cancel, so the system prompt stays on screen until the user answers it, and a new call gets `ERR_BIOMETRICS_IN_PROGRESS` until then | `stop_resolves_the_call_as_cancelled_and_the_late_answer_is_ignored`; "stop resolves the call as cancelled" in `auth.test.tsx` |
| `canCheckBiometrics` (getter) | `LocalAuth::can_check_biometrics()` | `LocalAuth.canCheckBiometrics()` | Done per platform: Apple `canEvaluatePolicy` with biometrics; Android `BiometricManager.canAuthenticate`; Windows `UserConsentVerifier`; Linux a fprintd enrolled finger; web `false`, an unsupported target | `the_device_queries_pass_through_the_backend`; the device answer is platform only |
| `isDeviceSupported()` | `LocalAuth::is_device_supported()` | `LocalAuth.isDeviceSupported()` | Done per platform: Apple `DeviceOwnerAuthentication`; Android `KeyguardManager.isDeviceSecure`; Windows Hello available; Linux polkit's authority answers; web `false`, an unsupported target | `the_device_queries_pass_through_the_backend` |
| `getAvailableBiometrics()` | `LocalAuth::available_biometrics()` | `LocalAuth.getAvailableBiometrics()` | Partial. Apple reports one kind (Face or Touch ID, or Optic ID as `iris`), and only when a biometric is enrolled; Android reports the strength class and the features present; Windows reports `strong` only; Linux `fingerprint`; web nothing, an unsupported target | `the_device_queries_pass_through_the_backend`; `each_kind_has_the_name_tsx_uses` |

## `BiometricType`

| `local_auth` | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `face` | `BiometricType::Face` | `"face"` | Done | `each_kind_has_the_name_tsx_uses` |
| `fingerprint` | `BiometricType::Fingerprint` | `"fingerprint"` | Done | `each_kind_has_the_name_tsx_uses` |
| `iris` | `BiometricType::Iris` | `"iris"` | Done (Apple Optic ID; Android iris feature) | `each_kind_has_the_name_tsx_uses` |
| `strong` | `BiometricType::Strong` | `"strong"` | Done (Android class 3; Windows Hello) | `each_kind_has_the_name_tsx_uses` |
| `weak` | `BiometricType::Weak` | `"weak"` | Done (Android class 2) | `each_kind_has_the_name_tsx_uses` |

## `LocalAuthExceptionCode`

Flutter reports these as `LocalAuthException` codes. This library reports the codes in the right column. The mapping is in `crates/nsh-biometrics/src/platform/robius.rs` (`map_error`) and `crates/nsh-biometrics/src/error.rs`.

| `local_auth` code | Code here | Status | Covered by |
|---|---|---|---|
| `authInProgress` | `ERR_BIOMETRICS_IN_PROGRESS` | Done | `a_second_prompt_is_refused_while_one_runs` |
| `uiUnavailable` | `ERR_BIOMETRICS_NO_ACTIVITY` | Done. Android with no running application; Apple and Linux when the system cannot show the UI | `each_robius_error_maps_to_one_code` (`NotInteractive`) |
| `userCanceled` | `ERR_BIOMETRICS_CANCELLED` | Done | `every_kind_reaches_the_caller_with_its_code` |
| `timeout` | `ERR_BIOMETRICS_TIMEOUT` | Done. robius's `Timeout` carries the code, and the opt-in `timeout` option resolves with it | `each_robius_error_maps_to_one_code` |
| `systemCanceled` | `ERR_BIOMETRICS_CANCELLED` | Done | `each_robius_error_maps_to_one_code` |
| `noCredentialsSet` | `ERR_BIOMETRICS_PASSCODE_NOT_SET` | Partial. Android folds `NO_DEVICE_CREDENTIAL` into robius's `NotAvailable`, so the library reports `ERR_BIOMETRICS_NOT_AVAILABLE` there. Unfixable without our own Android callback | `every_kind_reaches_the_caller_with_its_code` |
| `noBiometricsEnrolled` | `ERR_BIOMETRICS_NOT_ENROLLED` | Done | `each_robius_error_maps_to_one_code` (`NotEnrolled`, `NotConfigured`) |
| `noBiometricHardware` | `ERR_BIOMETRICS_NOT_AVAILABLE` | Done | `each_robius_error_maps_to_one_code` (`Unavailable`) |
| `biometricHardwareTemporarilyUnavailable` | `ERR_BIOMETRICS_NOT_AVAILABLE` | Done | `each_robius_error_maps_to_one_code` (`Unavailable`) |
| `temporaryLockout` | `ERR_BIOMETRICS_LOCKED_OUT` | Done | `each_robius_error_maps_to_one_code` (`Exhausted`) |
| `biometricLockout` | `ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT` | Partial. robius-authentication maps Android's permanent lockout to `Exhausted`, so the library reports `ERR_BIOMETRICS_LOCKED_OUT` on every platform. The code exists and the fake backend returns it | `every_kind_reaches_the_caller_with_its_code` |
| `userRequestedFallback` | `ERR_BIOMETRICS_CANCELLED` | Done. Apple's biometric-only case: the user tapped the passcode-fallback button and backed out, which is a user cancel | `each_robius_error_maps_to_one_code` |
| `deviceError` | `ERR_BIOMETRICS_BACKEND` | Done, with the platform status where robius gives one. robius 0.3.1 gives none on Apple, so `status()` is empty there | `a_platform_status_is_kept_with_its_error` |
| `unknownError` | `ERR_BIOMETRICS_BACKEND` | Done | `each_robius_error_maps_to_one_code` (`Unknown`) |

The library adds three codes Flutter does not have: `ERR_BIOMETRICS_INVALID_ARGUMENT` (an empty reason, or a Linux call without a polkit action), `ERR_BIOMETRICS_NOT_AVAILABLE` on the web, which `authenticate` returns, as `docs/brief.md` asks, and `ERR_BIOMETRICS_TIMEOUT`, which the opt-in `timeout` option resolves with.

## `robius-authentication` 0.3.1 features

| Feature | Here | Status | Covered by |
|---|---|---|---|
| Apple: Face ID, Touch ID and the passcode through `LAContext` | the prompt, `platform/robius.rs` | Done, but Face ID needs `NSFaceIDUsageDescription`, which NSH 0.4.0 cannot write (see `docs/decisions.md`) | Platform only |
| Apple: `companion` (Apple Watch) and `wrist_detection` | `companion(false)`; wrist detection not set | Not used. The watch prompt is not offered | none |
| Android: `BiometricPrompt`, API 28+ | the prompt, `platform/robius.rs` | Done | Platform only |
| Android: the device-credential fallback, API 29+ | `password` set from `biometric_only` | Done. On API 28 the credential fallback is off, as robius does | Platform only |
| Android: `BiometricStrength` | `AuthOptions::biometric_strength` | Done | `to_auth_options` unit test (`native.rs`) |
| Android: title and subtitle | `title`, `subtitle` | Done. The description is not exposed | `the_reason_and_the_polkit_action_reach_the_backend` |
| Android: the negative button text | `negative_button` | Accepted. robius uses the system's Cancel | none beyond the field's pass-through |
| Windows Hello, with the CredUI fallback | the prompt | Done | Platform only |
| Windows: caption and message length | `WindowsText::new_truncated` | Done. Long text is cut at a character boundary | Platform only |
| Linux: polkit `CheckAuthorization` | the prompt, with `polkit_action_id` | Done. robius's own note says Linux support is incomplete, and the action must exist in an installed policy file | Platform only |
| `action_ids` (polkit) | `polkit_action_id` | Done. Required on Linux | `a_policy_without_a_polkit_action_is_refused` (`robius.rs`) |
| `Error` variants | `map_error` | Done. Every variant maps to a code | `each_robius_error_maps_to_one_code`; `a_failed_match_is_an_answer_and_a_pass_is_true` |
| `Context::authenticate` callback | one answer, through `done` | Done. Taken once, under a mutex | `a_call_resolves_once_with_the_system_answer` |
| Cancel a prompt | not offered by robius | Missing. See `stopAuthentication` | `dropping_the_call_cancels_the_prompt` (the library side) |
