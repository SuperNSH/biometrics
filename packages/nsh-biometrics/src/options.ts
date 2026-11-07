/** A kind of biometric, as `getAvailableBiometrics` names it. */
export type BiometricType = "face" | "fingerprint" | "iris" | "strong" | "weak";

/** Android's strength class. Other platforms ignore it. */
export type BiometricStrength = "strong" | "weak";

/** The options of one authentication, with local_auth's names where it has them. */
export interface AuthOptions {
  /** Only biometrics may unlock: no device passcode or password fallback. */
  biometricOnly?: boolean;
  /** Declared as a sensitive transaction. Accepted; the prompt is the same. */
  sensitiveTransaction?: boolean;
  /** Keep the prompt across a pause. Accepted; no platform resumes it. */
  stickyAuth?: boolean;
  /** Flutter's option. Accepted; the platforms show their own error text. */
  useErrorDialogs?: boolean;
  /** The Android prompt's title. Defaults to the reason. */
  title?: string;
  /** The Android prompt's subtitle. */
  subtitle?: string;
  /** The Android strength class. Defaults to strong. */
  biometricStrength?: BiometricStrength;
  /** The polkit action to check on Linux. Required there. */
  polkitActionId?: string;
}
