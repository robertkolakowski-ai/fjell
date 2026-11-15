// Robusthet (kontraktfasen, docs/v1/ROBUSTHET-KONTRAKT.md): plassholderskjermene mot MockBridge. Innstillinger har sikkerhetskopi,
// indeksering og diagnose; trygg modus vises når indeksen ikke er hel (`?mockIntegrity=corrupt`, bare dev); gjenoppretting krever
// avkrysning og kan angres. Axe i lys og mørk, bred og smal, og pseudo-lokalet kuttes ikke.
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { open } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

async function axe(page: Page, where: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const res = await page.evaluate(async (tags) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; nodes: { target: unknown[] }[] }) => `${v.id}: ${v.nodes.slice(0, 3).map((n) => n.target.join(" ")).join(" | ")}`);
  }, TAGS);
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

test("Innstillinger har sikkerhetskopi, indeksering og diagnose med personvernnotat", async ({ page }) => {
  await open(page, "#/settings");
  await expect(page.getByRole("group", { name: "Sikkerhetskopi av indeksen" })).toBeVisible();
  await expect(page.getByRole("group", { name: "Indeksering" })).toBeVisible();
  await expect(page.getByRole("group", { name: "Diagnose" })).toBeVisible();
  await expect(page.getByText("Fjell sender ingenting noe sted", { exact: false })).toBeVisible();
  await expect(page.getByRole("list", { name: "Kopier av indeksen" }).getByRole("listitem")).toHaveCount(2);
  // Rapporten har bare koder og tall.
  const text = await page.getByLabel("Slik ser rapporten ut").inputValue();
  expect(text).toMatch(/^format=1\nintegrity=ok\nindexing=idle\n/);
  expect(text).not.toMatch(/[\\/:]/);
});

test("Ta kopi nå legger til en kopi og annonserer det", async ({ page }) => {
  await open(page, "#/settings");
  await page.getByRole("button", { name: "Ta kopi nå" }).click();
  await expect(page.getByRole("list", { name: "Kopier av indeksen" }).getByRole("listitem")).toHaveCount(3);
});

test("trygg modus: gjenoppretting krever avkrysning, kan angres, og «Sjekk på nytt» åpner appen igjen", async ({ page }) => {
  await open(page, "#/", "nb", 0, "&mockIntegrity=corrupt");
  await expect(page.getByRole("heading", { level: 1, name: "Trygg modus" })).toBeVisible();
  await expect(page.getByRole("alert").filter({ hasText: "Indeksen er ikke hel" })).toBeVisible();
  await page.getByRole("button", { name: /Gjenopprett kopien: Egen kopi/ }).click();
  const dialog = page.getByRole("dialog");
  const confirm = dialog.getByRole("button", { name: "Gjenopprett", exact: true });
  await expect(confirm).toBeDisabled();
  await dialog.getByRole("checkbox", { name: "Jeg forstår at indeksen erstattes" }).check();
  await expect(confirm).toBeEnabled();
  await confirm.click();
  await expect(page.locator(".banner.ok")).toContainText("Indeksen er gjenopprettet.");
  await page.getByRole("button", { name: "Angre" }).click();
  await expect(page.locator("p.integrity")).toHaveText("Indeksen er ødelagt.");
  // Gjenopprett på nytt og gå videre.
  await page.getByRole("button", { name: /Gjenopprett kopien: Egen kopi/ }).click();
  await dialog.getByRole("checkbox").check();
  await dialog.getByRole("button", { name: "Gjenopprett", exact: true }).click();
  await page.getByRole("button", { name: "Sjekk på nytt" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Trygg modus" })).toHaveCount(0);
  await expect(page.locator("#main")).toBeVisible();
});

test("Avbryt i bekreftelsen gjenoppretter ingenting", async ({ page }) => {
  await open(page, "#/", "nb", 0, "&mockIntegrity=degraded");
  await page.getByRole("button", { name: /Gjenopprett kopien: Automatisk kopi/ }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Avbryt" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("heading", { level: 1, name: "Trygg modus" })).toBeVisible();
  await expect(page.locator("p.integrity")).toHaveText("Indeksen har avvik.");
});

test("indeksering på batteri vises på Hjem", async ({ page }) => {
  await open(page, "#/", "nb", 0, "&mockScanPolls=1000&mockBattery=1");
  await page.getByRole("button", { name: "Start skanning" }).click();
  await expect(page.getByRole("group", { name: "Indeksering" }).getByText("Satt på pause fordi PC-en går på batteri.")).toBeVisible({ timeout: 10_000 });
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of [["bred", { width: 1280, height: 800 }], ["smal", { width: 360, height: 740 }]] as const) {
    test(`axe robusthet ${scheme} ${vp}: 0 brudd`, async ({ page }) => {
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await open(page, "#/settings");
      await page.getByRole("list", { name: "Kopier av indeksen" }).waitFor();
      await axe(page, `innstillinger ${scheme} ${vp}`);
      await open(page, "#/", "nb", 0, "&mockIntegrity=corrupt");
      await page.getByRole("heading", { level: 1, name: "Trygg modus" }).waitFor();
      await axe(page, `trygg modus ${scheme} ${vp}`);
    });
  }
}

test("pseudo-lokale: ingen horisontal rulling i Innstillinger og trygg modus", async ({ page }) => {
  for (const w of [1280, 360]) {
    await page.setViewportSize({ width: w, height: 800 });
    for (const q of ["", "&mockIntegrity=corrupt"]) {
      await open(page, "#/settings", "pseudo", 0, q);
      await page.waitForTimeout(200);
      const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(over, `${w}px ${q}`).toBeLessThanOrEqual(0);
    }
  }
});
