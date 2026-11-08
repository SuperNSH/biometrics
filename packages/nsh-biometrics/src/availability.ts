import { cleanup, effect, lifecycle, signal } from "nsh";
import { LocalAuth } from "./auth";
import { asBiometricsError, type BiometricsError } from "./errors";
import type { BiometricType } from "./options";

/** What the device can do for authentication right now. */
export interface Availability {
  deviceSupported: boolean;
  canCheckBiometrics: boolean;
  biometrics: BiometricType[];
}

/** Where the availability read stands. */
export type AvailabilityState =
  | { status: "checking" }
  | { status: "ready"; availability: Availability }
  | { status: "failed"; error: BiometricsError };

/** A reactive read of `Availability`. It checks at start, and again on each return to the foreground. */
export interface BiometricAvailability {
  state(): AvailabilityState;
  refresh(): void;
}

export interface UseBiometricAvailabilityOptions {
  /** The authentication to ask. Defaults to the Rust crate's. */
  auth?: LocalAuth;
}

// NSH 0.4.0's web runtime ships no `lifecycle` accessor: the import stays `undefined` there.
// Treat that as active, so the first read still runs; the foreground re-check comes back with an NSH release that has it.
function lifecycleState(): "active" | "inactive" | "background" {
  return typeof lifecycle === "function" ? lifecycle() : "active";
}

/**
 * Reads what the device can do, as a reactive value. A binding on `state()` runs again only when
 * the answer changes. The read ends when the component goes.
 */
export function useBiometricAvailability(options: UseBiometricAvailabilityOptions = {}): BiometricAvailability {
  const auth = options.auth ?? new LocalAuth();
  const state = signal<AvailabilityState>({ status: "checking" });
  let shown = "checking";
  let latest = 0;
  let gone = false;

  const show = (next: AvailabilityState) => {
    const key = describeKey(next);
    if (key === shown) return;
    shown = key;
    state.set(next);
  };

  const check = () => {
    // The handle can outlive its component, and a read then has no owner to land in.
    if (gone) return;
    const attempt = ++latest;
    readAll(auth).then(
      (next) => {
        if (attempt === latest) show(next);
      },
      (error: unknown) => {
        if (attempt === latest) show({ status: "failed", error: asBiometricsError(error) });
      },
    );
  };

  effect(() => {
    if (lifecycleState() === "active") check();
  });
  cleanup(() => {
    gone = true;
    latest += 1;
  });
  return { state: () => state(), refresh: check };
}

async function readAll(auth: LocalAuth): Promise<AvailabilityState> {
  const deviceSupported = await auth.isDeviceSupported();
  const canCheckBiometrics = await auth.canCheckBiometrics();
  const biometrics = await auth.getAvailableBiometrics();
  return { status: "ready", availability: { deviceSupported, canCheckBiometrics, biometrics } };
}

function describeKey(state: AvailabilityState): string {
  if (state.status === "ready") return `ready ${JSON.stringify(state.availability)}`;
  if (state.status === "failed") return `failed ${state.error.code}`;
  return "checking";
}
