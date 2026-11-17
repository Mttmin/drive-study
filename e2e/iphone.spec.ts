import { expect, test, type Page } from "@playwright/test";

async function stubSpeech(page: Page) {
  await page.addInitScript(() => {
    const synth = window.speechSynthesis;
    if (!synth) return;
    synth.speak = (utterance: SpeechSynthesisUtterance) => {
      window.setTimeout(() => utterance.onend?.(new Event("end")), 20);
    };
  });
}

test("settings controls respond to taps on an iPhone", async ({ page }) => {
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("open-settings").click();

  const controls = [
    "answer-spoken",
    "answer-listen",
    "explain-always",
    "explain-short",
    "explain-never",
    "voice-downloaded",
    "voice-streamed",
    "voice-phone",
    "rate",
    "answer-seconds",
    "voice-select",
  ];
  for (const id of controls) {
    const box = await page.getByTestId(id).boundingBox();
    expect(box, id).toBeTruthy();
    expect(box!.width, id).toBeGreaterThanOrEqual(44);
    expect(box!.height, id).toBeGreaterThanOrEqual(44);
  }

  await page.getByTestId("answer-listen").click();
  await expect(page.getByTestId("answer-listen")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("answer-spoken").click();
  await expect(page.getByTestId("answer-spoken")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("answer-listen")).toHaveAttribute("aria-pressed", "false");

  await page.getByTestId("explain-never").click();
  await expect(page.getByTestId("explain-never")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("explain-always").click();
  await expect(page.getByTestId("explain-always")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("explain-short").click();
  await expect(page.getByTestId("explain-short")).toHaveAttribute("aria-pressed", "true");

  await page.getByTestId("voice-phone").click();
  await expect(page.getByTestId("voice-phone")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("voice-streamed").click();
  await expect(page.getByTestId("voice-streamed")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("voice-downloaded").click();
  await expect(page.getByTestId("voice-downloaded")).toHaveAttribute("aria-pressed", "true");

  await page.getByTestId("answer-seconds").fill("15");
  await expect(page.getByTestId("answer-seconds-label")).toContainText("15");
  await page.getByTestId("rate").fill("1.5");
  await expect(page.getByTestId("rate-label")).toContainText("1.5");
});

test("the answer window stays open and a correct answer can skip the long explanation", async ({ page }) => {
  test.setTimeout(90000);
  await page.addInitScript(() => {
    const synth = window.speechSynthesis;
    if (!synth) return;
    synth.speak = (utterance: SpeechSynthesisUtterance) => {
      window.setTimeout(() => utterance.onend?.(new Event("end")), 30);
    };
  });
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("open-settings").click();
  await page.getByTestId("voice-phone").click();
  await page.getByTestId("rate").fill("2");
  await page.goto("./");
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await expect(page.getByTestId("choice-A")).toBeVisible();

  const window = page.getByTestId("answer-window");
  await expect(window).toBeVisible({ timeout: 45000 });
  const before = Number((await window.innerText()).match(/\d+/)?.[0]);
  expect(before).toBeGreaterThan(5);
  await page.waitForTimeout(1200);
  const mid = Number((await window.innerText()).match(/\d+/)?.[0]);
  expect(mid).toBeGreaterThan(0);
  expect(mid).toBeLessThan(before);
  await window.click();
  await expect.poll(async () => Number((await window.innerText()).match(/\d+/)?.[0])).toBeGreaterThan(mid);

  await page.getByTestId("choice-A").click();
  const status = page.getByTestId("status");
  await expect(status).toContainText(/Correct|Not this one|Here is the answer/);
  const text = await status.innerText();
  const card = page.getByTestId("explanation");
  await expect(card).toBeVisible();
  if (text.includes("Correct")) {
    await expect(card).toHaveAttribute("data-explain", "short");
  } else {
    await expect(card).toHaveAttribute("data-explain", "full");
  }
  const index = await page.locator(".player-top").innerText();
  await page.getByTestId("next").click();
  await expect(page.locator(".player-top")).not.toHaveText(index);
});

test("answers survive a reload and a miss is due immediately", async ({ page }) => {
  await stubSpeech(page);
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await page.getByTestId("dont-know").click();
  await expect(page.getByTestId("explanation")).toBeVisible();
  await page.waitForTimeout(300);
  await page.evaluate(() => {
    location.hash = "#/stats";
  });
  await expect(page.getByTestId("stats-count")).toContainText(/[1-9]\d* answers saved/);
  await page.reload();
  await expect(page.getByTestId("stats-count")).toContainText(/[1-9]\d* answers saved/);
  await page.evaluate(() => {
    location.hash = "#/mistakes";
  });
  await expect(page.getByTestId("mistakes-start")).toBeVisible();
});

test("listen-only reveals the answer when the window ends", async ({ page }) => {
  await stubSpeech(page);
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("open-settings").click();
  await page.getByTestId("answer-listen").click();
  await page.getByTestId("answer-seconds").fill("6");
  await page.evaluate(() => {
    location.hash = "#/";
  });
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await expect(page.getByTestId("answer-window")).toBeVisible({ timeout: 30000 });
  await expect(page.getByTestId("explanation")).toBeVisible({ timeout: 15000 });
});

test("the first rapid-fire card accepts Don't know immediately", async ({ page }) => {
  await stubSpeech(page);
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("mode-rapid").click();
  await page.getByRole("button", { name: "Start rapid fire" }).click();
  await page.getByTestId("dont-know").click();
  await expect(page.getByTestId("explanation")).toBeVisible({ timeout: 3000 });
});

test("the selected voice is marked and a missing clip says why", async ({ page }) => {
  await stubSpeech(page);
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("open-settings").click();
  await page.getByTestId("voice-streamed").click();
  await expect(page.getByTestId("voice-streamed")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("voice-streamed")).toContainText("Selected");
  await page.evaluate(() => {
    location.hash = "#/";
  });
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await expect(page.getByTestId("source")).toContainText(/Streamed clip|No streamed clip/);
});
