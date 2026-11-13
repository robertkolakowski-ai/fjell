// Oppdateringstest (hele flyten i appen). Egen konfig, adskilt fra playwright.real.config.ts. Se docs/mvp/OPPDATERING-E2E.md.
// Kjør fra desktop/: npx playwright test -c e2e-real/playwright.update.config.ts (krever miljøvariablene workflowen setter).
import { join } from "node:path";
import { defineConfig } from "@playwright/test";

const out = process.env.FJELL_UPDATE_OUT ?? "./.results/update";
export default defineConfig({
  testDir: ".",
  testMatch: /update\.spec\.ts$/,
  outputDir: join(out, "pw"),
  timeout: 600_000,
  workers: 1,
  retries: 0,
  reporter: [["list"], ["html", { open: "never", outputFolder: join(out, "html") }]],
  use: { screenshot: "only-on-failure", trace: "off" },
});
