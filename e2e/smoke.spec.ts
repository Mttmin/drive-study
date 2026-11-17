import { expect, test } from "@playwright/test";

test("answers a sample quiz question and still loads offline", async ({ page, context }) => {
  await page.goto("./");
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
  await page.getByTestId("mode-quiz").click();
  await page.getByTestId("start-quiz").click();
  await expect(page.getByTestId("choice-A")).toBeVisible();
  await page.getByTestId("choice-A").click();
  await expect(page.getByTestId("explanation")).toBeVisible();

  await page.goto("./");
  await page.waitForFunction(async () => {
    const registration = await navigator.serviceWorker?.getRegistration();
    return Boolean(registration?.active);
  });
  await page.reload();
  await context.setOffline(true);
  await page.reload();
  await expect(page.getByTestId("home-title")).toHaveText("Drive Study");
});
