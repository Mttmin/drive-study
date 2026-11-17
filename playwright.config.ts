import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  timeout: 60000,
  use: {
    baseURL: "http://127.0.0.1:4173/drive-study/",
    serviceWorkers: "allow",
  },
  webServer: {
    command: "npm run build && npm run preview -- --port 4173 --strictPort",
    port: 4173,
    reuseExistingServer: false,
    timeout: 180000,
  },
  projects: [
    {
      name: "webkit-iphone",
      use: { ...devices["iPhone 14"], browserName: "webkit" },
    },
  ],
});
