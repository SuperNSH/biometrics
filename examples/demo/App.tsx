import { mount, signal, Text, View } from "nsh";
import { BiometricsError, LocalAuth, useBiometricAvailability } from "../../packages/nsh-biometrics/src";

/** The device's own authentication, through the Rust crate. */
const auth = new LocalAuth();

/** What the last press showed: the answer, or the error's code. */
const result = signal("Not checked yet");

function codeOf(error: unknown): string {
  return error instanceof BiometricsError ? error.code : "unknown error";
}

function App() {
  const availability = useBiometricAvailability({ auth });
  const authenticate = () => {
    result.set("Waiting for the prompt");
    auth
      .authenticate({
        localizedReason: "Unlock your notes",
        options: { polkitActionId: "dev.supernsh.biometrics.demo" },
      })
      .then(
        (passed) => result.set(passed ? "Unlocked" : "Not unlocked"),
        (error: unknown) => result.set(codeOf(error)),
      );
  };
  return (
    <View class="flex-1 gap-4 p-4">
      <Text testID="availability" class="text-foreground">
        {availability.state().status === "ready" ? "Biometrics can be checked" : "Checking biometrics"}
      </Text>
      <Text testID="result" class="text-foreground">{result()}</Text>
      <Text testID="authenticate" role="button" onPress={authenticate} class="rounded-(--radius) bg-muted px-3 py-2">
        Authenticate
      </Text>
    </View>
  );
}

mount(App);
