import { expect, test, type Page } from "@playwright/test";
import { signIn } from "./sign-in";

function decodeSync(code: string) {
  const payload = code.slice(4).replace(/-/g, "+").replace(/_/g, "/");
  const pad = payload + "=".repeat((4 - (payload.length % 4)) % 4);
  return Buffer.from(pad, "base64").toString("utf8");
}

async function savedLesson(page: Page) {
  return page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open("drive-study");
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    const rows = await new Promise<Array<{ lessons?: Array<{ lessonId?: string; lesson_id?: string; status?: string }> }>>((resolve) => {
      if (!db.objectStoreNames.contains("profiles")) {
        resolve([]);
        return;
      }
      const tx = db.transaction("profiles", "readonly");
      const request = tx.objectStore("profiles").getAll();
      request.onsuccess = () => resolve(request.result || []);
      request.onerror = () => resolve([]);
    });
    db.close();
    return rows.some((row) => (row.lessons || []).some((lesson) => (lesson.lessonId || lesson.lesson_id) === "sie-1-01"));
  });
}

test("lesson, flashcard, and quiz progress survive reload, profiles, and transfer", async ({ page, browser }) => {
  test.setTimeout(120000);
  await page.addInitScript(() => {
    const synth = window.speechSynthesis;
    const voice = { voiceURI: "stub", name: "Stub", lang: "en-US", default: true, localService: true };
    Object.defineProperty(synth, "speaking", { configurable: true, get: () => false });
    synth.getVoices = () => [voice as SpeechSynthesisVoice];
    synth.cancel = () => {};
    synth.pause = () => {};
    synth.resume = () => {};
    synth.speak = (utterance: SpeechSynthesisUtterance) => {
      window.setTimeout(() => utterance.onend?.(new Event("end")), 5);
    };
  });
  await signIn(page, "Matt");
  await page.getByTestId("mode-lessons").click();
  await page.getByTestId("lesson-open-sie-1-01").click();
  await page.getByTestId("play-lesson").click();
  await expect(page.getByTestId("choice-A")).toBeVisible({ timeout: 20000 });
  const indexText = (await page.getByTestId("player-index").innerText()).trim();
  expect(indexText.startsWith("0")).toBeFalsy();
  await page.getByTestId("choice-A").click();
  await expect(page.getByTestId("explanation")).toBeVisible();
  await page.getByTestId("end-session").click();
  await expect(page.getByTestId("summary-score")).toBeVisible();

  await page.getByRole("button", { name: "Home" }).click();
  await page.getByTestId("mode-rapid").click();
  await page.getByTestId("start-rapid").click();
  await expect(page.getByTestId("player-index")).toContainText("1 /");
  await page.getByTestId("know-it").click();
  await expect(page.getByTestId("status")).toContainText("1 right");
  await page.getByTestId("end-session").click();
  await expect(page.getByTestId("summary-score")).toBeVisible();
  await expect.poll(() => savedLesson(page)).toBe(true);

  await page.evaluate(() => localStorage.removeItem("drive-study-disk-v2"));
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("mode-stats").click();
  await expect(page.getByTestId("stats-count")).toContainText("Quiz answers: 1");
  await expect(page.getByTestId("stats-count")).toContainText("Matt");
  await expect(page.getByTestId("stats-flashcards")).toContainText("Flashcard reviews: 1");
  await expect(page.getByTestId("stats-lessons")).toContainText("1 in progress");
  await expect(page.getByTestId("lesson-row-sie-1-01")).toContainText("Part");

  await page.getByTestId("lesson-row-sie-1-01").click();
  await page.getByTestId("resume-lesson").click();
  await expect(page.getByTestId("player-index")).toHaveText(indexText);
  await page.getByTestId("end-session").click();

  await page.getByTestId("open-profile").click();
  await page.getByTestId("new-profile-name").fill("Sam");
  await page.getByTestId("create-profile").click();
  await expect(page.getByTestId("open-profile")).toHaveText("Sam");
  await page.getByTestId("mode-stats").click();
  await expect(page.getByTestId("stats-lessons")).toContainText("0 finished, 0 in progress");
  await expect(page.getByTestId("stats-count")).toContainText("Quiz answers: 0");
  await expect(page.getByTestId("lesson-row-sie-1-01")).toHaveCount(0);

  await page.getByTestId("open-profile").click();
  await page.getByTestId("profile-Matt").click();
  await expect(page.getByTestId("open-profile")).toHaveText("Matt");
  await page.getByTestId("mode-stats").click();
  await expect(page.getByTestId("lesson-row-sie-1-01")).toBeVisible();

  await page.getByTestId("open-profile").click();
  await page.getByTestId("show-sync").click();
  await expect(page.getByTestId("sync-code")).not.toHaveValue("");
  const code = await page.getByTestId("sync-code").inputValue();
  expect(code.startsWith("DS2.")).toBeTruthy();

  const imported = await browser.newContext();
  const page2 = await imported.newPage();
  await page2.goto("http://127.0.0.1:4173/drive-study/");
  await page2.getByTestId("import-code").fill(code);
  await page2.getByTestId("import-sync").click();
  await expect(page2.getByTestId("open-profile")).toHaveText("Matt");
  await page2.getByTestId("mode-stats").click();
  await expect(page2.getByTestId("lesson-row-sie-1-01")).toContainText("Part");
  await expect(page2.getByTestId("stats-count")).toContainText("Quiz answers: 1");
  await expect(page2.getByTestId("stats-flashcards")).toContainText("Flashcard reviews: 1");
  await imported.close();

  const fileContext = await browser.newContext();
  const page3 = await fileContext.newPage();
  await page3.goto("http://127.0.0.1:4173/drive-study/");
  await page3.getByTestId("import-file").setInputFiles({
    name: "drive-study-Matt.json",
    mimeType: "application/json",
    buffer: Buffer.from(decodeSync(code)),
  });
  await expect(page3.getByTestId("open-profile")).toHaveText("Matt");
  await page3.getByTestId("mode-stats").click();
  await expect(page3.getByTestId("stats-flashcards")).toContainText("Flashcard reviews: 1");
  await expect(page3.getByTestId("lesson-row-sie-1-01")).toBeVisible();
  await fileContext.close();
});

test("old unkeyed progress is kept on the first profile", async ({ page }) => {
  test.setTimeout(60000);
  let seeded = false;
  await page.route("**/*", async (route) => {
    const url = route.request().url();
    if (route.request().resourceType() !== "document" || !url.includes("/drive-study") || url.includes("legacy=1") || seeded) {
      await route.continue();
      return;
    }
    seeded = true;
    await route.fulfill({
      status: 200,
      contentType: "text/html",
      body: `<!doctype html><meta charset="utf-8"><body><script>
        const req = indexedDB.open("drive-study", 1);
        req.onupgradeneeded = () => {
          const db = req.result;
          if (!db.objectStoreNames.contains("settings")) db.createObjectStore("settings");
          if (!db.objectStoreNames.contains("srs")) db.createObjectStore("srs", { keyPath: "id" });
          if (!db.objectStoreNames.contains("attempts")) db.createObjectStore("attempts", { autoIncrement: true });
          if (!db.objectStoreNames.contains("days")) db.createObjectStore("days");
        };
        req.onsuccess = () => {
          const db = req.result;
          const tx = db.transaction(["settings", "attempts", "days"], "readwrite");
          tx.objectStore("settings").put({
            plan: "SIE",
            answerMode: "spoken",
            voiceSource: "phone",
            rate: 1,
            pauseSeconds: 6,
            answerSeconds: 10,
            explainCorrect: "short",
            voiceUri: null,
            seenIntro: true,
            completedLessons: ["sie-1-02"]
          }, "app");
          tx.objectStore("attempts").add({
            itemId: "sie-q-0009",
            kind: "question",
            exam: "SIE",
            section: "SIE-1",
            topic: "Regulators",
            correct: true,
            guessed: false,
            mode: "Quiz",
            at: 1700000000000
          });
          tx.objectStore("days").put({ date: "2026-11-17", count: 2 }, "2026-11-17");
          tx.oncomplete = () => { db.close(); location.replace("/drive-study/?legacy=1"); };
        };
      </script></body>`,
    });
  });
  await page.goto("./");
  await expect(page.getByTestId("legacy-note")).toBeVisible({ timeout: 30000 });
  await page.getByTestId("profile-name").fill("Matt");
  await page.getByTestId("profile-create").click();
  await expect(page.getByTestId("home-title")).toBeVisible();
  await page.getByTestId("mode-stats").click();
  await expect(page.getByTestId("stats-count")).toContainText("Quiz answers: 1");
  await expect(page.getByTestId("lesson-row-sie-1-02")).toContainText("Finished");
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toBeVisible();
  await page.getByTestId("mode-stats").click();
  await expect(page.getByTestId("lesson-row-sie-1-02")).toContainText("Finished");
  await expect(page.getByTestId("stats-count")).toContainText("Quiz answers: 1");
});
