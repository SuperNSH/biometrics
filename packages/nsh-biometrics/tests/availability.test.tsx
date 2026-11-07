import { expect, open, test } from "nsh/test";

test("the answer is read at start, and again on each return to the foreground", async () => {
  const app = await open("./fixtures/availability.tsx", { clock: {} });
  await app.waitFor("text:availability supported true enrolled true face runs 2");
  await app.setLifecycle("background");
  await app.setLifecycle("active");
  await app.waitFor("text:reads 2");
  expect(app.texts()).toContain("availability supported true enrolled true face runs 2");
});

test("a binding runs again only when the answer changes", async () => {
  const app = await open("./fixtures/availability.tsx", { clock: {} });
  await app.waitFor("text:availability supported true enrolled true face runs 2");
  await app.press("id:remove");
  await app.setLifecycle("background");
  await app.setLifecycle("active");
  await app.waitFor("text:availability supported true enrolled false none runs 3");
});
