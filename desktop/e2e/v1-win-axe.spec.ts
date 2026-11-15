// V1 på Windows, axe i alle tre temaer: lys, mørk og høykontrast (forced-colors: active, emulert).
// Dekker alle V1-skjermer og V1-dialoger. Nye flater, ikke en kopi av a11y.spec.ts (som kjører lys/mørk × bred/smal).
// Merk: emulert forced-colors er ikke et ekte Windows-kontrasttema; ekte temaer sjekkes manuelt (docs/v1/WINDOWS-V1.md).
import { expect, test, type Page } from "@playwright/test";
import { go } from "./helpers";
import { axe, scanned, V1_SKJERMER } from "./v1-win-felles";

const TEMA = [
  { navn: "lys", opts: { colorScheme: "light" as const, forcedColors: "none" as const } },
  { navn: "mørk", opts: { colorScheme: "dark" as const, forcedColors: "none" as const } },
  { navn: "høykontrast (forced-colors, lys)", opts: { colorScheme: "light" as const, forcedColors: "active" as const } },
  { navn: "høykontrast (forced-colors, mørk)", opts: { colorScheme: "dark" as const, forcedColors: "active" as const } },
];

/** Åpner V1-dialogene én etter én og kjører axe mens hver er åpen. Esc lukker. */
async function dialoger(page: Page, tema: string): Promise<void> {
  const åpne = async (hash: string, knapp: string | RegExp, tittel: string | RegExp) => {
    await go(page, hash);
    await page.getByRole("button", { name: knapp }).first().click();
    await expect(page.getByRole("dialog", { name: tittel })).toBeVisible();
    await axe(page, `${tema}: dialog ${String(tittel)}`);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toBeHidden();
  };
  await åpne("#/rules", "Ny regel", "Ny regel");
  await åpne("#/rules", "Rediger regelen Store filer", "Rediger regel");
  await åpne("#/people", "Slå på ansiktsindeks", "Slå på ansiktsindeks?");
  await åpne("#/settings", "Beskytt en mappe", "Beskytt en mappe");
  await åpne("#/cards/series", "Hurtigtaster", /Hurtigtaster/);
}

for (const t of TEMA) {
  test(`axe, alle V1-skjermer og dialoger: ${t.navn}`, async ({ page }) => {
    test.setTimeout(150_000);
    await page.emulateMedia({ ...t.opts, reducedMotion: "reduce" });
    await scanned(page, "#/");
    if (t.opts.forcedColors === "active") expect(await page.evaluate(() => matchMedia("(forced-colors: active)").matches)).toBe(true);
    for (const s of V1_SKJERMER) {
      await go(page, s.hash);
      await page.waitForTimeout(100);
      await axe(page, `${t.navn}: ${s.navn}`);
    }
    await dialoger(page, t.navn);
  });
}

test("axe: Personer med ansiktsindeksen på, og Innstillinger med indeksen på (lys og høykontrast)", async ({ page }) => {
  test.setTimeout(90_000);
  await scanned(page, "#/people");
  await page.getByRole("button", { name: "Slå på ansiktsindeks" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Slå på", exact: true }).click();
  await expect(page.getByRole("list", { name: "Personer" }).getByRole("listitem")).toHaveCount(4);
  for (const forcedColors of ["none", "active"] as const) {
    await page.emulateMedia({ forcedColors, colorScheme: "light", reducedMotion: "reduce" });
    await go(page, "#/people");
    await axe(page, `personer på, forcedColors=${forcedColors}`);
    await go(page, "#/settings");
    await axe(page, `innstillinger med indeks på, forcedColors=${forcedColors}`);
  }
});

test("axe: engelsk (en) i lys og høykontrast på alle V1-skjermer", async ({ page }) => {
  test.setTimeout(120_000);
  await scanned(page, "#/", "en");
  for (const forcedColors of ["none", "active"] as const) {
    await page.emulateMedia({ forcedColors, colorScheme: "light", reducedMotion: "reduce" });
    for (const s of V1_SKJERMER) {
      await go(page, s.hash);
      await page.waitForTimeout(100);
      await axe(page, `en, forcedColors=${forcedColors}: ${s.navn}`);
    }
  }
});
