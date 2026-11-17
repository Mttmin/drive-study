import { expect, test } from "@playwright/test";

test("listen-only still counts down when speech never ends", async ({ page }) => {
  test.setTimeout(90000);
  await page.addInitScript(() => {
    const synth = window.speechSynthesis;
    const voice = { voiceURI: "stub", name: "Stub", lang: "en-US", default: true, localService: true };
    Object.defineProperty(synth, "speaking", { configurable: true, get: () => true });
    Object.defineProperty(synth, "pending", { configurable: true, get: () => true });
    synth.getVoices = () => [voice as SpeechSynthesisVoice];
    synth.speak = () => {};
    synth.cancel = () => {};
    synth.pause = () => {};
    synth.resume = () => {};
  });
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("open-settings").click();
  await page.getByTestId("answer-listen").click();
  await page.getByTestId("voice-phone").click();
  await page.getByTestId("answer-seconds").fill("6");
  await page.getByTestId("rate").fill("2");
  await page.evaluate(() => {
    location.hash = "#/";
  });
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await expect(page.getByTestId("answer-window")).toBeVisible({ timeout: 40000 });
  await expect(page.getByTestId("explanation")).toBeVisible({ timeout: 15000 });
});
