import { expect, test, type Page } from "@playwright/test";
import { signIn } from "./sign-in";

type RecStep =
  | { type: "end"; delay?: number }
  | { type: "nospeech"; delay?: number }
  | { type: "result"; text: string; delay?: number };

async function installSpeech(
  page: Page,
  options: { steps?: RecStep[]; holdUntilResume?: boolean; emptyVoices?: boolean; stuckSpeaking?: boolean } = {},
) {
  const steps = options.steps ?? [];
  await page.addInitScript(
    ({ steps, holdUntilResume, emptyVoices, stuckSpeaking }) => {
      const synth = window.speechSynthesis;
      const voice = { voiceURI: "stub", name: "Stub", lang: "en-US", default: true, localService: true };
      const log: Array<{ text: string; cancelGap: number }> = [];
      const cancels: number[] = [];
      let lastCancel = 0;
      let paused = false;
      let pending = false;
      const held: SpeechSynthesisUtterance[] = [];
      (window as unknown as { __speaks: typeof log; __cancels: number[]; __hold: boolean; __starts: number }).__speaks = log;
      (window as unknown as { __cancels: number[] }).__cancels = cancels;
      (window as unknown as { __hold: boolean }).__hold = Boolean(holdUntilResume);
      (window as unknown as { __starts: number }).__starts = 0;

      Object.defineProperty(synth, "paused", { configurable: true, get: () => paused });
      Object.defineProperty(synth, "pending", { configurable: true, get: () => pending });
      Object.defineProperty(synth, "speaking", { configurable: true, get: () => Boolean(stuckSpeaking) });
      synth.getVoices = () => (emptyVoices ? [] : [voice as SpeechSynthesisVoice]);
      synth.cancel = () => {
        lastCancel = performance.now();
        cancels.push(lastCancel);
        pending = false;
        paused = false;
      };
      const finishUtterance = (utterance: SpeechSynthesisUtterance) => {
        const end = new Event("end");
        Object.defineProperty(end, "elapsedTime", { value: 0.4 });
        try { utterance.dispatchEvent(new Event("start")); } catch { /* WebKit */ }
        try { utterance.onstart?.(new Event("start")); } catch { /* WebKit */ }
        try { utterance.dispatchEvent(end); } catch { /* WebKit */ }
        try { utterance.onend?.(end); } catch { /* WebKit */ }
      };
      const release = () => {
        const utterance = held.shift();
        if (!utterance) return;
        pending = false;
        paused = false;
        window.setTimeout(() => finishUtterance(utterance), 15);
      };
      synth.pause = () => {
        paused = true;
      };
      synth.resume = () => {
        paused = false;
        release();
      };
      synth.speak = (utterance: SpeechSynthesisUtterance) => {
        const now = performance.now();
        log.push({ text: utterance.text, cancelGap: lastCancel ? now - lastCancel : 9999, at: Date.now() });
        lastCancel = 0;
        if ((window as unknown as { __hold?: boolean }).__hold) {
          held.push(utterance);
          pending = true;
          paused = true;
          return;
        }
        window.setTimeout(() => finishUtterance(utterance), 20);
      };

      class FakeRecognition {
        lang = "en-US";
        continuous = false;
        interimResults = false;
        maxAlternatives = 1;
        onresult: ((event: unknown) => void) | null = null;
        onend: (() => void) | null = null;
        onerror: ((event: unknown) => void) | null = null;
        start() {
          const slot = window as unknown as { __starts: number; __steps: RecStep[] };
          const index = slot.__starts++;
          const step = slot.__steps[index] ?? ({ type: "end", delay: 40 } as RecStep);
          const delay = step.delay ?? 40;
          window.setTimeout(() => {
            if (step.type === "result") {
              const alternative = { transcript: step.text, confidence: 0.92 };
              const result = [alternative] as Array<{ transcript: string; confidence: number }> & { isFinal: boolean };
              result.isFinal = true;
              this.onresult?.({ results: [result] });
              this.onend?.();
            } else if (step.type === "nospeech") {
              this.onerror?.({ error: "no-speech" });
              this.onend?.();
            } else {
              this.onend?.();
            }
          }, delay);
        }
        abort() {
          this.onend?.();
        }
        stop() {
          this.onend?.();
        }
      }
      (window as unknown as { __steps: RecStep[] }).__steps = steps;
      const typed = window as unknown as { webkitSpeechRecognition: typeof FakeRecognition; SpeechRecognition: typeof FakeRecognition };
      typed.webkitSpeechRecognition = FakeRecognition;
      typed.SpeechRecognition = FakeRecognition;
    },
    { steps, holdUntilResume: options.holdUntilResume ?? false, emptyVoices: options.emptyVoices ?? false, stuckSpeaking: options.stuckSpeaking ?? false },
  );
}

async function speaks(page: Page) {
  return page.evaluate(() => (window as unknown as { __speaks: Array<{ text: string; cancelGap: number }> }).__speaks);
}

function spokenLines(rows: Array<{ text: string }>) {
  return rows.map((row) => row.text.trim()).filter((text) => text.length > 1);
}

test("a quiz speaks each line once and scores a spoken answer", async ({ page }) => {
  test.setTimeout(90000);
  await installSpeech(page, {
    steps: [{ type: "result", text: "bravo", delay: 350 }],
  });
  await signIn(page);
  await page.getByTestId("open-settings").click();
  await page.getByTestId("voice-phone").click();
  await page.getByTestId("explain-short").click();
  await page.evaluate(() => {
    location.hash = "#/";
  });
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  const listening = page.getByTestId("answer-window");
  await expect(listening).toBeVisible({ timeout: 20000 });
  await expect(listening).toHaveAttribute("data-listening", "true");
  await expect(listening).toContainText(/Listening/);
  const beforeAnswer = spokenLines(await speaks(page));
  const counts = new Map<string, number>();
  for (const text of beforeAnswer) counts.set(text, (counts.get(text) ?? 0) + 1);
  for (const [text, count] of counts) {
    expect(count, text).toBe(1);
  }
  for (let index = 1; index < beforeAnswer.length; index += 1) {
    expect(beforeAnswer[index]).not.toBe(beforeAnswer[index - 1]);
  }
  const gaps = await page.evaluate(() =>
    (window as unknown as { __speaks: Array<{ text: string; cancelGap: number }> }).__speaks
      .filter((row) => row.text.trim().length > 1)
      .map((row) => row.cancelGap),
  );
  for (const gap of gaps) expect(gap).toBeGreaterThan(40);

  const heardAt = Date.now();
  await expect(page.getByTestId("status")).toContainText(/Correct|Not this one/, { timeout: 2000 });
  expect(Date.now() - heardAt).toBeLessThan(2000);
});

test("recognition restarts after an early end and a missed answer is asked once", async ({ page }) => {
  test.setTimeout(90000);
  await installSpeech(page, {
    steps: [
      { type: "end", delay: 30 },
      { type: "result", text: "b", delay: 40 },
    ],
  });
  await signIn(page);
  await page.getByTestId("open-settings").click();
  await page.getByTestId("voice-phone").click();
  await page.evaluate(() => {
    location.hash = "#/";
  });
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await expect(page.getByTestId("answer-window")).toBeVisible({ timeout: 20000 });
  await expect(page.getByTestId("status")).toContainText(/1 (right|missed)/, { timeout: 8000 });
  const starts = await page.evaluate(() => (window as unknown as { __starts: number }).__starts);
  expect(starts).toBeGreaterThanOrEqual(2);
});

test("the listening countdown keeps moving", async ({ page }) => {
  test.setTimeout(90000);
  await installSpeech(page, { steps: [] });
  await signIn(page);
  await page.getByTestId("open-settings").click();
  await page.getByTestId("voice-phone").click();
  await page.getByTestId("answer-seconds").fill("6");
  await page.evaluate(() => {
    location.hash = "#/";
  });
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  const listening = page.getByTestId("answer-window");
  await expect(listening).toBeVisible({ timeout: 20000 });
  const firstLeft = Number((await listening.innerText()).match(/\d+/)?.[0]);
  await page.waitForTimeout(1500);
  const laterLeft = Number((await listening.innerText()).match(/\d+/)?.[0]);
  expect(laterLeft).toBeLessThan(firstLeft);
});
