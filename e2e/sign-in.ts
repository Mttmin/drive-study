import { expect, type Page } from "@playwright/test";

export async function signIn(page: Page, name = "Matt") {
  await page.goto("./");
  const gate = page.getByTestId("profile-name");
  const home = page.getByTestId("home-title");
  await expect(gate.or(home)).toBeVisible({ timeout: 30000 });
  if (await gate.isVisible()) {
    await gate.fill(name);
    await page.getByTestId("profile-create").click();
  }
  await expect(home).toHaveText("Drive Study", { timeout: 30000 });
}
