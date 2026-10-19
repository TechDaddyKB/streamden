import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}-{platform}{ext}",
  reporter: "list",
  use: { baseURL: "http://127.0.0.1:5184", colorScheme: "dark" },
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.01 } },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 900, height: 640 } } }],
  webServer: {
    command: "pnpm build && pnpm preview",
    url: "http://127.0.0.1:5184",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
