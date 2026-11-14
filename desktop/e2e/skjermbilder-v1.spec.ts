// Skjermbilder av V1-skjermene til docs/screens/v1/ (JPEG, nb, lys og mørk, bred 1280 × 800 og smal 480 × 800).
// Kjøres bare med SHOTS=1:  SHOTS=1 npx playwright test e2e/skjermbilder-v1.spec.ts
// Navn: <skjerm>-nb-<light|dark>-<bred|smal>.jpg. Mappen kan overstyres med SHOTS_DIR.
import { mkdirSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { go, open, seedScan } from "./helpers";

const OUT = process.env.SHOTS_DIR ?? "../docs/screens/v1";
test.skip(!process.env.SHOTS, "bare med SHOTS=1");

const SIZES = { bred: { width: 1280, height: 800 }, smal: { width: 480, height: 800 } } as const;

for (const scheme of ["light", "dark"] as const) {
  for (const [size, vp] of Object.entries(SIZES)) {
    test(`V1-skjermbilder nb ${scheme} ${size}`, async ({ page }) => {
      test.setTimeout(180_000);
      mkdirSync(OUT, { recursive: true });
      const tag = `nb-${scheme}-${size}`;
      const shot = async (name: string, full = false): Promise<void> => {
        await page.waitForTimeout(300);
        await page.screenshot({ path: `${OUT}/${name}-${tag}.jpg`, type: "jpeg", quality: 60, fullPage: full });
      };
      await page.setViewportSize(vp);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await open(page, "#/", "nb");
      await seedScan(page);

      await go(page, "#/");
      await page.locator(".home button").first().waitFor();
      await page.waitForTimeout(300);
      await shot("hjem-oversikt", true);

      await go(page, "#/review/series");
      await page.locator("main h1").first().waitFor();
      await shot("gjennomgang-serier");

      await go(page, "#/series/41");
      await page.locator("ul.items img, ul.items .img").first().waitFor({ timeout: 10_000 }).catch(() => undefined);
      await shot("serie");

      await go(page, "#/cards/series");
      await page.locator(".deck").waitFor();
      await shot("kortmodus-serie");
      await go(page, "#/cards/rule");
      await page.locator(".deck").waitFor();
      await shot("kortmodus-regel");

      await go(page, "#/rules");
      await page.locator(".list .item").first().waitFor();
      await shot("regler");
      await page.locator("main .btn.primary").first().click();
      await expect(page.getByRole("dialog")).toBeVisible();
      await shot("regler-ny-regel");
      await page.keyboard.press("Escape");

      await go(page, "#/people");
      await shot("personer-av");
      await page.locator("main .btn.primary").first().click();
      await expect(page.getByRole("dialog")).toBeVisible();
      await shot("personer-samtykke");
      await page.locator("dialog[open] .btn.primary").last().click();
      await page.locator("ul.grid .person").first().waitFor();
      await shot("personer", true);

      await go(page, "#/settings");
      await page.waitForTimeout(200);
      await shot("innstillinger-v1", true);
    });
  }
}
