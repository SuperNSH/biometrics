import type { AuthOptions, BiometricType } from "./options";

/** What a `LocalAuth` runs on: the Rust crate, or a memory backend in tests. */
export interface AuthBackend {
  isDeviceSupported(): Promise<boolean>;
  canCheckBiometrics(): Promise<boolean>;
  getAvailableBiometrics(): Promise<BiometricType[]>;
  /** Resolves once, with `true` when the user passed and `false` when they failed. */
  authenticate(reason: string, options: AuthOptions): Promise<boolean>;
  /** Cancels the running prompt. Resolves `true` when one was running. */
  stopAuthentication(): Promise<boolean>;
}
