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
   * a lockout, a timeout or no enrollment rejects with a `BiometricsError` that has its code. A
   * second call while one runs rejects with `ERR_BIOMETRICS_IN_PROGRESS`, and a zero or invalid
   * `timeoutMs` with `ERR_BIOMETRICS_INVALID_ARGUMENT`.
   */
  authenticate({ localizedReason, options = {} }: AuthenticateArgs): Promise<boolean> {
    if (localizedReason.trim() === "" || (options.timeoutMs !== undefined && !(options.timeoutMs > 0))) {
      return Promise.reject(biometricsError("ERR_BIOMETRICS_INVALID_ARGUMENT"));
    }
    if (this.busy) {
      return Promise.reject(biometricsError("ERR_BIOMETRICS_IN_PROGRESS"));
    }
    this.busy = true;
    return new Promise<boolean>((resolve, reject) => {
      let settled = false;
      let timer: ReturnType<typeof setTimeout> | null = null;
      const settle = (outcome: Outcome) => {
        if (settled) return;
        settled = true;
        if (timer !== null) clearTimeout(timer);
        if (this.waiting === settle) this.waiting = null;
        if (outcome.ok) resolve(outcome.value);
        else reject(outcome.error);
      };
      this.waiting = settle;
      if (options.timeoutMs !== undefined) {
        timer = setTimeout(() => {
          // Stop keeps the gate closed until the backend answers, so only an owned gate frees here.
          if (this.waiting === settle) {
            this.busy = false;
            this.waiting = null;
          }
          settle({ ok: false, error: biometricsError("ERR_BIOMETRICS_TIMEOUT") });
        }, options.timeoutMs);
      }
      let prompt: Promise<boolean>;
      try {
        prompt = this.backend.authenticate(localizedReason, options);
      } catch (error: unknown) {
        // A backend may throw before returning its promise; that rejects the call like any failure.
        prompt = Promise.reject(error);
      }
      // The backend answer frees the gate unless a newer call owns it now.
      const answerFreesGate = () => this.waiting === settle || this.waiting === null;
      prompt.then(
        (value) => {
          if (answerFreesGate()) this.busy = false;
          settle({ ok: true, value });
        },
        (error: unknown) => {
          if (answerFreesGate()) this.busy = false;
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
