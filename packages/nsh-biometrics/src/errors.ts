/** The codes a biometrics rejection carries. Apps match on `code`. */
export type BiometricsErrorCode =
  | "ERR_BIOMETRICS_NOT_AVAILABLE"
  | "ERR_BIOMETRICS_NOT_ENROLLED"
  | "ERR_BIOMETRICS_LOCKED_OUT"
  | "ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT"
  | "ERR_BIOMETRICS_PASSCODE_NOT_SET"
  | "ERR_BIOMETRICS_CANCELLED"
  | "ERR_BIOMETRICS_IN_PROGRESS"
  | "ERR_BIOMETRICS_NO_ACTIVITY"
  | "ERR_BIOMETRICS_BACKEND"
  | "ERR_BIOMETRICS_INVALID_ARGUMENT";

/** Same wording as the Rust crate's messages. None holds the reason text. */
const MESSAGES: Record<BiometricsErrorCode, string> = {
  ERR_BIOMETRICS_NOT_AVAILABLE: "Biometrics and device credentials are not available on this device.",
  ERR_BIOMETRICS_NOT_ENROLLED: "No biometric is enrolled. Add one in the system settings.",
  ERR_BIOMETRICS_LOCKED_OUT: "Too many failed attempts. Try again later or use the device passcode.",
  ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT:
    "Biometrics are locked. Sign in with the device passcode to unlock them.",
  ERR_BIOMETRICS_PASSCODE_NOT_SET: "Set a device passcode in the system settings first.",
  ERR_BIOMETRICS_CANCELLED: "The authentication was cancelled.",
  ERR_BIOMETRICS_IN_PROGRESS: "Another authentication is already running. Wait for it to finish.",
  ERR_BIOMETRICS_NO_ACTIVITY: "There is no screen to show the prompt on right now.",
  ERR_BIOMETRICS_BACKEND: "The platform's authentication service failed.",
  ERR_BIOMETRICS_INVALID_ARGUMENT: "An argument is empty or not allowed on this platform.",
};

const KNOWN = new Set<string>(Object.keys(MESSAGES));

/** A rejection of the biometrics API. `code` is the stable name; the message says what to do. */
export class BiometricsError extends Error {
  readonly code: BiometricsErrorCode;

  constructor(code: BiometricsErrorCode) {
    super(MESSAGES[code]);
    this.name = "BiometricsError";
    this.code = code;
  }
}

/** The error for a known code. */
export function biometricsError(code: BiometricsErrorCode): BiometricsError {
  return new BiometricsError(code);
}

/**
 * Turns whatever the native side or a host threw into a coded `BiometricsError`. A host's
 * rejection carries `ERR_NSH_HOST_ERROR`, and its message starts with the code, as
 * `ERR_BIOMETRICS_CANCELLED: text`. Anything unknown is a backend failure.
 */
export function asBiometricsError(error: unknown): BiometricsError {
  if (error instanceof BiometricsError) return error;
  const raw = (error ?? {}) as { code?: unknown; message?: unknown };
  const code = typeof raw.code === "string" ? raw.code : "";
  const message = typeof raw.message === "string" ? raw.message : "";
  if (code === "ERR_NSH_HOST_ERROR") {
    const named = /^(ERR_BIOMETRICS_[A-Z_]+): /.exec(message)?.[1];
    return named !== undefined && KNOWN.has(named)
      ? biometricsError(named as BiometricsErrorCode)
      : biometricsError("ERR_BIOMETRICS_BACKEND");
  }
  return KNOWN.has(code) ? biometricsError(code as BiometricsErrorCode) : biometricsError("ERR_BIOMETRICS_BACKEND");
}
