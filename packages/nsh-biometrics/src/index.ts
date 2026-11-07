export { LocalAuth, type AuthenticateArgs } from "./auth";
export { type AuthBackend } from "./backend";
export { BiometricsError, asBiometricsError, biometricsError, type BiometricsErrorCode } from "./errors";
export { memoryBackend, type MemoryAuth, type MemoryAuthConfig } from "./memory";
export { type AuthOptions, type BiometricStrength, type BiometricType } from "./options";
export {
  type Availability,
  type AvailabilityState,
  type BiometricAvailability,
  type UseBiometricAvailabilityOptions,
  useBiometricAvailability,
} from "./availability";
export { rustBackend } from "./rust";
