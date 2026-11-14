// Skjermbilder av V2-skallet til docs/screens/m11/ (JPEG, 1280 × 800, lys og mørk). Kjøres bare med SHOTS=1:
//   SHOTS=1 npx playwright test e2e/skjermbilder-v2.spec.ts
import { mkdirSync } from "node:fs";
import { test, type Page } from "@playwright/test";
import { go, open, seedScan } from "./helpers";

const OUT = process.env.SHOTS_DIR ?? "../docs/screens/m11";
test.skip(!process.env.SHOTS, "bare med SHOTS=1");

async function shot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${OUT}/${name}.jpg`, type: "jpeg", quality: 55 });
}

for (const scheme of ["light", "dark"] as const) {
  test(`V2-skjermbilder ${scheme}`, async ({ page }) => {
    test.setTimeout(120_000);
    mkdirSync(OUT, { recursive: true });
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
    await open(page, "#/", "nb");
    await seedScan(page);
    await go(page, "#/");
    await page.locator(".home button").first().waitFor();
    await shot(page, `hjem-${scheme}`);
    await go(page, "#/group/1");
    await page.locator("img").first().waitFor({ timeout: 10_000 }).catch(() => undefined);
    await shot(page, `gruppe-${scheme}`);
    await go(page, "#/review/screenshot");
    await page.locator(".tile").first().waitFor();
    await page.locator(".tile").nth(0).click();
    await page.locator(".tile").nth(2).click();
    await page.locator(".bar button").last().click();
    await page.locator(".bar [role=status]").first().waitFor();
    await go(page, "#/trash");
    await page.locator(".trash .summary").waitFor();
    await shot(page, `papirkurv-${scheme}`);
    await go(page, "#/settings");
    await shot(page, `innstillinger-${scheme}`);
  });
}
