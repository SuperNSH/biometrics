import { expect, test } from "nsh/test";
import { LocalAuth, asBiometricsError, memoryBackend, type BiometricsErrorCode } from "../src";

/** Lets the answer of a finished backend call reach the gate, which frees itself a microtask later. */
async function delivered(): Promise<void> {
  for (let turn = 0; turn < 4; turn += 1) await Promise.resolve();
}

const CODES: BiometricsErrorCode[] = [
  "ERR_BIOMETRICS_NOT_AVAILABLE",
  "ERR_BIOMETRICS_NOT_ENROLLED",
  "ERR_BIOMETRICS_LOCKED_OUT",
  "ERR_BIOMETRICS_PERMANENTLY_LOCKED_OUT",
  "ERR_BIOMETRICS_PASSCODE_NOT_SET",
  "ERR_BIOMETRICS_CANCELLED",
  "ERR_BIOMETRICS_IN_PROGRESS",
  "ERR_BIOMETRICS_NO_ACTIVITY",
  "ERR_BIOMETRICS_BACKEND",
  "ERR_BIOMETRICS_INVALID_ARGUMENT",
];

test("one prompt runs at a time, and the answer resolves its call once", async () => {
  const backend = memoryBackend();
  const auth = new LocalAuth({ backend });
  const first = auth.authenticate({ localizedReason: "Unlock your notes" });
  await expect(auth.authenticate({ localizedReason: "Again" })).rejects.toMatchObject({
    code: "ERR_BIOMETRICS_IN_PROGRESS",
  });
  expect(backend.prompts()).toBe(1);
  expect(backend.finishPrompt(true)).toBe(true);
  expect(await first).toBe(true);
  expect(backend.finishPrompt(false)).toBe(false);
});

test("a decline resolves false, not an error", async () => {
  const backend = memoryBackend();
  const auth = new LocalAuth({ backend });
  const call = auth.authenticate({ localizedReason: "Unlock" });
  backend.finishPrompt(false);
  expect(await call).toBe(false);
});

test("stop resolves the call as cancelled, and a late answer is ignored", async () => {
  const backend = memoryBackend();
  const auth = new LocalAuth({ backend });
  const first = auth.authenticate({ localizedReason: "Unlock" });
  expect(await auth.stopAuthentication()).toBe(true);
  await expect(first).rejects.toMatchObject({ code: "ERR_BIOMETRICS_CANCELLED" });
  expect(backend.cancels()).toBe(1);

  // The system still shows the prompt until it answers, so a new call waits for that answer.
  await expect(auth.authenticate({ localizedReason: "Again" })).rejects.toMatchObject({
    code: "ERR_BIOMETRICS_IN_PROGRESS",
  });
  backend.finishPrompt(true);
  await delivered();
  const next = auth.authenticate({ localizedReason: "Next" });
  backend.finishPrompt(true);
  expect(await next).toBe(true);
});

test("stop with no prompt running reports false", async () => {
  const auth = new LocalAuth({ backend: memoryBackend() });
  expect(await auth.stopAuthentication()).toBe(false);
});

test("an empty reason is refused without a prompt", async () => {
  const backend = memoryBackend();
  const auth = new LocalAuth({ backend });
  await expect(auth.authenticate({ localizedReason: "   " })).rejects.toMatchObject({
    code: "ERR_BIOMETRICS_INVALID_ARGUMENT",
  });
  expect(backend.prompts()).toBe(0);
});

test("every code reaches the caller as its code", async () => {
  const backend = memoryBackend();
  const auth = new LocalAuth({ backend });
  for (const code of CODES) {
    if (code === "ERR_BIOMETRICS_IN_PROGRESS") continue;
    const call = auth.authenticate({ localizedReason: "Unlock" });
    backend.finishPrompt(code);
    await expect(call).rejects.toMatchObject({ code });
    await delivered();
  }
});

test("a native rejection keeps its code, and an unknown one is a backend failure", () => {
  expect(asBiometricsError({ code: "ERR_BIOMETRICS_LOCKED_OUT", message: "x" }).code).toBe("ERR_BIOMETRICS_LOCKED_OUT");
  expect(asBiometricsError({ code: "ERR_NSH_HOST_ERROR", message: "ERR_BIOMETRICS_CANCELLED: text" }).code).toBe(
    "ERR_BIOMETRICS_CANCELLED",
  );
  expect(asBiometricsError(new Error("boom")).code).toBe("ERR_BIOMETRICS_BACKEND");
});

test("options and the reason reach the backend, and the device queries pass through", async () => {
  const backend = memoryBackend({ biometrics: ["face", "fingerprint"] });
  const auth = new LocalAuth({ backend });
  const call = auth.authenticate({ localizedReason: "Open", options: { biometricOnly: true, polkitActionId: "x" } });
  expect(backend.lastReason()).toBe("Open");
  expect(backend.lastOptions()).toEqual({ biometricOnly: true, polkitActionId: "x" });
  backend.finishPrompt(true);
  expect(await call).toBe(true);
  expect(await auth.getAvailableBiometrics()).toEqual(["face", "fingerprint"]);
  expect(await auth.canCheckBiometrics()).toBe(true);
});
