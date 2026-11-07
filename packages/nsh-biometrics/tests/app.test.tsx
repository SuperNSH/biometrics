import { open, test } from "nsh/test";

test("the screen opens with its title", async () => {
  const app = await open("../App.tsx", { clock: {} });
  await app.waitFor("text:nsh-biometrics");
});
