import { mount, signal, Text, View } from "nsh";
import { type AvailabilityState, BiometricsError, LocalAuth, useBiometricAvailability } from "./src";

/** The device's own authentication, through the Rust crate. */
const auth = new LocalAuth();

const message = signal("nothing yet");

function describe(state: AvailabilityState): string {
  if (state.status === "checking") return "availability checking";
  if (state.status === "failed") return `availability failed ${state.error.code}`;
  const { deviceSupported, canCheckBiometrics, biometrics } = state.availability;
  return `availability supported ${deviceSupported} enrolled ${canCheckBiometrics} ${biometrics.join(",") || "none"}`;
}

function codeOf(error: unknown): string {
  return error instanceof BiometricsError ? error.code : "unknown error";
}

function App() {
  const availability = useBiometricAvailability({ auth });
  const stop = () => {
    auth.stopAuthentication().then(
      (stopped) => message.set(stopped ? "stopped the prompt" : "No prompt was running"),
      (error: unknown) => message.set(codeOf(error)),
    );
  };
  return (
    <View class="flex-1 gap-3 p-4">
      <Text testID="title" class="text-foreground">nsh-biometrics</Text>
      <Text testID="availability" class="text-foreground">{describe(availability.state())}</Text>
      <Text testID="message" class="text-foreground">{message()}</Text>
      <Text testID="stop" role="button" onPress={stop} class="rounded-(--radius) bg-muted px-3 py-2">
        Stop
      </Text>
    </View>
  );
}

mount(App);
