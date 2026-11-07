import { NativeLocalAuth, type NativeAuthOptions } from "nsh-biometrics/native";
import type { AuthBackend } from "./backend";
import { asBiometricsError } from "./errors";
import type { AuthOptions, BiometricType } from "./options";

/**
 * The Rust crate's authentication (`nsh-biometrics/native`). The app or package must list the
 * crate under `"rust"` in `nsh.json`; importing this module without it fails the bundle.
 */
export function rustBackend(): AuthBackend {
  return new RustBackend();
}

/** Only the options the crate reads: `useErrorDialogs` is Flutter's and has no effect here. */
function nativeOptions(options: AuthOptions): NativeAuthOptions {
  const result: NativeAuthOptions = {};
  if (options.biometricOnly !== undefined) result.biometricOnly = options.biometricOnly;
  if (options.sensitiveTransaction !== undefined) result.sensitiveTransaction = options.sensitiveTransaction;
  if (options.stickyAuth !== undefined) result.stickyAuth = options.stickyAuth;
  if (options.title !== undefined) result.title = options.title;
  if (options.subtitle !== undefined) result.subtitle = options.subtitle;
  if (options.negativeButton !== undefined) result.negativeButton = options.negativeButton;
  if (options.fallbackTitle !== undefined) result.fallbackTitle = options.fallbackTitle;
  if (options.cancelTitle !== undefined) result.cancelTitle = options.cancelTitle;
  if (options.biometricStrength !== undefined) result.biometricStrength = options.biometricStrength;
  if (options.polkitActionId !== undefined) result.polkitActionId = options.polkitActionId;
  return result;
}

class RustBackend implements AuthBackend {
  private readonly native = new NativeLocalAuth();

  isDeviceSupported(): Promise<boolean> {
    return this.settle(() => this.native.isDeviceSupported());
  }

  canCheckBiometrics(): Promise<boolean> {
    return this.settle(() => this.native.canCheckBiometrics());
  }

  getAvailableBiometrics(): Promise<BiometricType[]> {
    return this.settle(async () => (await this.native.availableBiometrics()) as BiometricType[]);
  }

  authenticate(reason: string, options: AuthOptions): Promise<boolean> {
    return this.settle(() => this.native.authenticate(reason, nativeOptions(options)));
  }

  stopAuthentication(): Promise<boolean> {
    return this.settle(() => this.native.stopAuthentication());
  }

  private async settle<T>(operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (error) {
      throw asBiometricsError(error);
    }
  }
}
