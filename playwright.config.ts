import { defineConfig, devices } from "@playwright/test";

const iphone = { ...devices["iPhone 14"] };
// PW_USE_CHROME=1 runs the iPhone layout on installed Google Chrome when
// Playwright's WebKit build is not available.
const browser = process.env.PW_USE_CHROME
  ? { browserName: "chromium" as const, channel: "chrome" }
  : { browserName: "webkit" as const };

export default defineConfig({
  testDir: "e2e",
  timeout: 60000,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173/drive-study/",
    serviceWorkers: "allow",
  },
  webServer: {
    command: "python3 scripts/serve-dist.py",
    port: 4173,
    reuseExistingServer: !process.env.CI,
    timeout: 30000,
  },
  projects: [
    {
      name: "webkit-iphone",
      testIgnore: "**/listen-stuck.spec.ts",
      use: { ...iphone, ...browser },
    },
    {
      name: "webkit-no-tts",
      testMatch: "**/listen-stuck.spec.ts",
      use: { ...iphone, ...browser },
    },
    {
      name: "chromium-no-tts",
      testMatch: "**/listen-stuck.spec.ts",
      use: {
        browserName: "chromium",
        channel: process.env.PW_USE_CHROME ? "chrome" : undefined,
        viewport: { width: 390, height: 844 },
        hasTouch: true,
      },
    },
  ],
});
