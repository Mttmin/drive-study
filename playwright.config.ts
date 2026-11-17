import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  timeout: 60000,
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
      use: { ...devices["iPhone 14"], browserName: "webkit" },
    },
  ],
});
