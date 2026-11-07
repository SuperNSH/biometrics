import { mount, signal, Text, View } from "nsh";
import { type AvailabilityState, LocalAuth, memoryBackend, useBiometricAvailability } from "../../src";

/** How many times the device support was read. A signal, so the screen shows it. */
const reads = signal(0);
// Counted outside the signal: reading it in the check would subscribe the check's effect to it.
let readCount = 0;
const backend = memoryBackend({
  deviceSupported: true,
  canCheckBiometrics: true,
  biometrics: ["face"],
  onSupportRead: () => {
    readCount += 1;
    reads.set(readCount);
  },
});
const auth = new LocalAuth({ backend });

function describe(state: AvailabilityState): string {
  if (state.status === "checking") return "availability checking";
  if (state.status === "failed") return `availability failed ${state.error.code}`;
  const { deviceSupported, canCheckBiometrics, biometrics } = state.availability;
  return `availability supported ${deviceSupported} enrolled ${canCheckBiometrics} ${biometrics.join(",") || "none"}`;
}

function App() {
  const availability = useBiometricAvailability({ auth });
  let runs = 0;
  // The line counts its own runs, so a test sees whether a read ran the binding again.
  const line = () => {
    runs += 1;
    return `${describe(availability.state())} runs ${runs}`;
  };
  return (
    <View class="flex-1 gap-3 p-4">
      <Text testID="availability" class="text-foreground">{line()}</Text>
      <Text testID="reads" class="text-foreground">{`reads ${reads()}`}</Text>
      <Text testID="remove" role="button" onPress={() => backend.setBiometrics([])} class="rounded-(--radius) bg-muted px-3 py-2">
        Remove
      </Text>
    </View>
  );
}

mount(App);
