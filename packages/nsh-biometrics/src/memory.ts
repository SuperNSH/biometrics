import type { AuthBackend } from "./backend";
import { biometricsError, type BiometricsErrorCode } from "./errors";
import type { AuthOptions, BiometricType } from "./options";

/** The answers a memory backend gives. Each check answers what the config says. */
export interface MemoryAuthConfig {
  deviceSupported?: boolean;
  canCheckBiometrics?: boolean;
  biometrics?: BiometricType[];
  /** Called on each read of `isDeviceSupported`, so a test can count the checks. */
  onSupportRead?: () => void;
}

/** A backend the test drives: it finishes prompts and counts what was asked. */
export interface MemoryAuth extends AuthBackend {
  /** Answers the waiting prompt: `true` passed, `false` declined, or a code to reject with. */
  finishPrompt(answer: boolean | BiometricsErrorCode): boolean;
  promptPending(): boolean;
  prompts(): number;
  cancels(): number;
  lastReason(): string | undefined;
  lastOptions(): AuthOptions | undefined;
  setDeviceSupported(value: boolean): void;
  setBiometrics(value: BiometricType[]): void;
}

/** A memory backend, for tests and for apps that run without a sensor. */
export function memoryBackend(config: MemoryAuthConfig = {}): MemoryAuth {
  let deviceSupported = config.deviceSupported ?? true;
  let canCheck = config.canCheckBiometrics ?? true;
  let biometrics: BiometricType[] = config.biometrics ?? ["face"];
  let waiting: ((answer: boolean | BiometricsErrorCode) => void) | null = null;
  let prompts = 0;
  let cancels = 0;
  let reason: string | undefined;
  let options: AuthOptions | undefined;

  return {
    isDeviceSupported: async () => {
      config.onSupportRead?.();
      return deviceSupported;
    },
    canCheckBiometrics: async () => canCheck,
    getAvailableBiometrics: async () => (canCheck ? [...biometrics] : []),
    authenticate: (nextReason: string, nextOptions: AuthOptions) => {
      prompts += 1;
      reason = nextReason;
      options = nextOptions;
      return new Promise<boolean>((resolve, reject) => {
        waiting = (answer) => {
          if (typeof answer === "boolean") resolve(answer);
          else reject(biometricsError(answer));
        };
      });
    },
    stopAuthentication: async () => {
      cancels += 1;
      return waiting !== null;
    },
    finishPrompt: (answer) => {
      const done = waiting;
      waiting = null;
      done?.(answer);
      return done !== null;
    },
    promptPending: () => waiting !== null,
    prompts: () => prompts,
    cancels: () => cancels,
    lastReason: () => reason,
    lastOptions: () => options,
    setDeviceSupported: (value) => {
      deviceSupported = value;
    },
    setBiometrics: (value) => {
      biometrics = [...value];
      canCheck = value.length > 0;
    },
  };
}
