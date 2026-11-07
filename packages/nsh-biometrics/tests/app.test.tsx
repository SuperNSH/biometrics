import { open, test } from "nsh/test";

test("the screen opens, and stopping with no prompt says so", async () => {
  const app = await open("../App.tsx", { clock: {} });
  await app.waitFor("text:nsh-biometrics");
  await app.press("id:stop");
  await app.waitFor("text:No prompt was running");
});
