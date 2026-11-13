// Skjermbilder til docs/screens/m7/ (kjøres bare med SHOTS=1: `SHOTS=1 npx playwright test e2e/screens.spec.ts`).
import { mkdirSync } from "node:fs";
import { test, type Page } from "@playwright/test";
import { go, open, seedScan, type Lang } from "./helpers";

const OUT = process.env.SHOTS_DIR ?? "../docs/screens/m7";
test.skip(!process.env.SHOTS, "bare med SHOTS=1");
test.use({ deviceScaleFactor: 2 });

async function shot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${OUT}/${name}.jpg`, type: "jpeg", quality: 60, fullPage: name.endsWith("200pct") });
}

async function tour(page: Page, lang: Lang, scheme: "light" | "dark", only?: string[]): Promise<void> {
  const tag = `${lang}-${scheme}`;
  const want = (n: string) => !only || only.includes(n);
  await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
  await open(page, "#/onboarding", lang);
  if (want("onboarding-velkommen")) await shot(page, `onboarding-velkommen-${tag}`);
  await page.locator(".onb > .row button").first().click();
  if (want("onboarding-mapper")) await shot(page, `onboarding-mapper-${tag}`);
  await page.locator(".onb > .row button").first().click();
  await page.locator("h1").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 });
  if (want("onboarding-resultat")) await shot(page, `onboarding-resultat-${tag}`);
  await go(page, "#/");
  await page.locator(".home button").first().waitFor();
  if (want("hjem")) await shot(page, `hjem-${tag}`);
  const stops: [string, string][] = [
    ["gjennomgang", "#/review"], ["gjennomgang-uskarpe", "#/review/blurry"], ["gjennomgang-eksakte", "#/review/exact_dup"],
    ["gruppe", "#/group/1"], ["kortmodus", "#/cards"],
  ];
  for (const [n, h] of stops) {
    await go(page, h);
    if (want(n)) await shot(page, `${n}-${tag}`);
  }
  // Valgt-tilstand i Gjennomgang
  await go(page, "#/review/screenshot");
  await page.locator(".tile").first().waitFor();
  await page.locator(".tile").nth(0).click();
  await page.locator(".tile").nth(2).click();
  if (want("gjennomgang-valgt")) await shot(page, `gjennomgang-valgt-${tag}`);
  await page.locator(".bar button").last().click();
  await page.locator(".bar [role=status]").first().waitFor();
  await go(page, "#/trash");
  await page.locator(".trash .summary").waitFor();
  if (want("papirkurv")) await shot(page, `papirkurv-${tag}`);
  await page.locator(".trash .ph .btn.danger").click();
  if (want("papirkurv-tom-ark")) await shot(page, `papirkurv-tom-ark-${tag}`);
  await page.keyboard.press("Escape");
  await go(page, "#/cards");
  await page.locator(".deck").waitFor();
  await page.keyboard.press("F1");
  if (want("kortmodus-hjelp")) await shot(page, `kortmodus-hjelp-${tag}`);
  await go(page, "#/settings");
  if (want("innstillinger")) await shot(page, `innstillinger-${tag}`);
}

for (const scheme of ["light", "dark"] as const) {
  test(`skjermbilder nb ${scheme}`, async ({ page }) => {
    test.setTimeout(180_000);
    mkdirSync(OUT, { recursive: true });
    await page.setViewportSize({ width: 900, height: 620 });
    await tour(page, "nb", scheme);
  });
}

test("skjermbilder pseudo (to skjermer, smal)", async ({ page }) => {
  test.setTimeout(120_000);
  mkdirSync(OUT, { recursive: true });
  await page.setViewportSize({ width: 420, height: 760 });
  await tour(page, "pseudo", "light", ["hjem", "gjennomgang-valgt", "papirkurv", "kortmodus", "gruppe"]);
});

test("skjermbilde Hjem 200 %", async ({ page }) => {
  mkdirSync(OUT, { recursive: true });
  await page.setViewportSize({ width: 450, height: 310 }); // 900x620 ved 200 % zoom
  await page.emulateMedia({ colorScheme: "light" });
  await open(page, "#/", "nb");
  await seedScan(page);
  await go(page, "#/");
  await page.locator(".home button").first().waitFor();
  await shot(page, "hjem-nb-light-200pct");
});
