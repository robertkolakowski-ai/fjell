// e2e mot vite dev med MockBridge (ingen Tauri). System-Chrome kan brukes med PW_CHROME=/sti/til/chrome.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  outputDir: "e2e/.results",
  timeout: 60_000,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:1420",
    launchOptions: process.env.PW_CHROME ? { executablePath: process.env.PW_CHROME } : {},
  },
  webServer: { command: "npm run dev", url: "http://localhost:1420", reuseExistingServer: !process.env.CI, timeout: 60_000 },
});
