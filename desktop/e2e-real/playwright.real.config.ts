// Ekte-app-e2e (Windows). Egen konfig, helt adskilt fra desktop/playwright.config.ts (mock-e2e).
// Kjør fra desktop/: npx playwright test -c e2e-real/playwright.real.config.ts
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: /.*\.spec\.ts/,
  outputDir: process.env.FJELL_REAL_RESULTS ?? "./.results/pw",
  timeout: 240_000,
  workers: 1,
  retries: 0,
  reporter: [["list"], ["html", { open: "never", outputFolder: "./.results/html" }]],
  use: { screenshot: "only-on-failure", trace: "off" },
});
