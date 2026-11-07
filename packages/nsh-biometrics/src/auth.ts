import type { AuthBackend } from "./backend";
import { asBiometricsError, biometricsError, type BiometricsError } from "./errors";
import { rustBackend } from "./rust";
import type { AuthOptions, BiometricType } from "./options";

/** The arguments of `authenticate`, with local_auth's names. */
export interface AuthenticateArgs {
  localizedReason: string;
  options?: AuthOptions;
}

type Outcome = { ok: true; value: boolean } | { ok: false; error: BiometricsError };

/**
 * Authentication with the device's biometrics or credentials, as `local_auth` names it. It runs
 * on the Rust crate unless it is given another backend. One prompt runs at a time, and each call
 * resolves exactly once.
 */
export class LocalAuth {
  private readonly backend: AuthBackend;
  private busy = false;
  private waiting: ((outcome: Outcome) => void) | null = null;

  constructor(options: { backend?: AuthBackend } = {}) {
    this.backend = options.backend ?? rustBackend();
  }

  isDeviceSupported(): Promise<boolean> {
    return this.backend.isDeviceSupported();
  }

  canCheckBiometrics(): Promise<boolean> {
    return this.backend.canCheckBiometrics();
  }

  getAvailableBiometrics(): Promise<BiometricType[]> {
    return this.backend.getAvailableBiometrics();
  }

  /**
   * Shows the prompt. Resolves `true` when the user passed, `false` when they failed. A cancel,
   * a lockout or no enrollment rejects with a `BiometricsError` that has its code. A second call
   * while one runs rejects with `ERR_BIOMETRICS_IN_PROGRESS`.
   */
  authenticate({ localizedReason, options = {} }: AuthenticateArgs): Promise<boolean> {
    if (localizedReason.trim() === "") {
      return Promise.reject(biometricsError("ERR_BIOMETRICS_INVALID_ARGUMENT"));
    }
    if (this.busy) {
      return Promise.reject(biometricsError("ERR_BIOMETRICS_IN_PROGRESS"));
    }
    this.busy = true;
    return new Promise<boolean>((resolve, reject) => {
      let settled = false;
      const settle = (outcome: Outcome) => {
        if (settled) return;
        settled = true;
        if (this.waiting === settle) this.waiting = null;
        if (outcome.ok) resolve(outcome.value);
        else reject(outcome.error);
      };
      this.waiting = settle;
      this.backend.authenticate(localizedReason, options).then(
        (value) => {
          this.busy = false;
          settle({ ok: true, value });
        },
        (error: unknown) => {
          this.busy = false;
          settle({ ok: false, error: asBiometricsError(error) });
        },
      );
    });
  }

  /**
   * Cancels the running prompt. The pending `authenticate` rejects with `ERR_BIOMETRICS_CANCELLED`.
   * Resolves `true` when a prompt was running. The system may still show it until it answers.
   */
  async stopAuthentication(): Promise<boolean> {
    const busy = this.busy;
    const waiting = this.waiting;
    this.waiting = null;
    waiting?.({ ok: false, error: biometricsError("ERR_BIOMETRICS_CANCELLED") });
    if (busy) await this.backend.stopAuthentication();
    return busy;
  }
}
