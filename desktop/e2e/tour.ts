// Felles omvisning (UX-6): besøker alle skjermer og viktige tilstander og kaller `visit(navn)` på hver.
// Brukes av høykontrast-, redusert-bevegelse-, skalerings- og ARIA-testene slik at de dekker samme flater.
import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";
import { go, open, seedScan, type Lang } from "./helpers";

export type Visit = (name: string) => Promise<void>;

export async function tour(page: Page, visit: Visit, lang: Lang = "nb"): Promise<void> {
  // Pseudo-lokalet har forvrengt tekst: der brukes strukturelle velgere (som i screens.spec.ts).
  const pseudo = lang === "pseudo";
  const startBtn = lang === "en" ? "Get started" : "Kom i gang";
  await open(page, "#/onboarding", lang);
  await visit("onboarding-velkommen");
  if (pseudo) await page.locator(".onb > .row button").first().click();
  else await page.getByRole("button", { name: startBtn }).click();
  await visit("onboarding-mapper");
  if (pseudo) await page.locator(".onb > .row button").last().click(); // «Legg til mappe» er først, «Start skanning» sist
  else await page.getByRole("button", { name: /Start skanning|Start scan/ }).click();
  if (pseudo) await page.locator("h1").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 });
  else await page.getByRole("button", { name: /^(Flytt|Move) \d+/ }).waitFor({ timeout: 15_000 });
  await visit("onboarding-resultat");

  await go(page, "#/");
  await page.locator("h1").first().waitFor();
  await page.waitForTimeout(150);
  await visit("hjem");

  for (const [hash, name] of [
    ["#/review", "gjennomgang"], ["#/review/blurry", "gjennomgang-uskarpe"], ["#/review/screenshot", "gjennomgang-skjermbilder"],
    ["#/review/exact_dup", "gjennomgang-eksakte"], ["#/group/1", "gruppe"], ["#/cards", "kortmodus"], ["#/settings", "innstillinger"],
  ] as const) {
    await go(page, hash);
    await page.waitForTimeout(120);
    await visit(name);
  }

  // Dialog: hurtigtaster (F1)
  await go(page, "#/cards");
  await page.keyboard.press("F1");
  await expect(page.getByRole("dialog")).toBeVisible();
  await visit("kortmodus-hjelp");
  await page.keyboard.press("Escape");

  // Valgt-tilstand med bunnlinje
  await go(page, "#/review/blurry");
  await page.locator(".tile").first().waitFor();
  await page.locator(".tile").nth(0).click();
  await page.locator(".tile").nth(2).click();
  await visit("gjennomgang-valgt");
  if (pseudo) await page.locator(".bar button").last().click();
  else await page.getByRole("button", { name: /^(Flytt|Move) \d+/ }).last().click();
  await page.locator(".bar [role=status]").first().waitFor();
  await page.waitForTimeout(150);
  await visit("gjennomgang-etter-flytt");

  await go(page, "#/trash");
  await page.locator("h1").first().waitFor();
  await page.waitForTimeout(150);
  await visit("papirkurv");
  const purge = pseudo ? page.locator(".trash .ph .btn.danger") : page.getByRole("button", { name: /Tøm papirkurv|Empty trash/ });
  if ((await purge.count()) > 0 && (await purge.first().isEnabled({ timeout: 1000 }))) {
    await purge.first().click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await visit("papirkurv-tom-ark");
    await page.keyboard.press("Escape");
  }
}

/** Skjermbilder lagres bare med SHOTS=1 (som i screens.spec.ts); mappen kan overstyres med SHOTS_DIR. */
export async function shot(page: Page, name: string): Promise<void> {
  if (!process.env.SHOTS) return;
  const dir = process.env.SHOTS_DIR ?? "../docs/screens/m8";
  await page.waitForTimeout(200);
  await page.screenshot({ path: `${dir}/${name}.jpg`, type: "jpeg", quality: 55, fullPage: false });
}
