// V1-skjermer, kontraktfasen: nye ruter, monteringspunkter, forslagstypene serier og regler og ProtectToggle. axe på hver ny flate.
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open, seedScan } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

async function axe(page: Page, where: string, include?: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const res = await page.evaluate(async ({ tags, include }) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(include ? { include: [[include]] } : document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; nodes: { target: unknown[] }[] }) => ({ id: v.id, nodes: v.nodes.slice(0, 3).map((n) => n.target.join(" ")) }));
  }, { tags: TAGS, include });
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

/** Åpner Hjem etter en skanning (mocken har da forslag i alle seks kategorier). */
async function scanned(page: Page): Promise<void> {
  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/");
  await page.getByRole("button", { name: "Rask opprydding" }).waitFor();
}

test("navigasjon: Personer og Regler er toppnivå, Serie hører til Gjennomgang", async ({ page }) => {
  await open(page, "#/");
  const nav = page.getByRole("navigation", { name: "Hovednavigasjon" });
  await expect(nav.getByRole("link")).toHaveText([/Hjem/, /Gjennomgang/, /Personer/, /Regler/, /Papirkurv/, /Innstillinger/]);
  await nav.getByRole("link", { name: /Personer/ }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Personer" })).toBeFocused();
  await expect(nav.getByRole("link", { name: /Personer/ })).toHaveAttribute("aria-current", "page");
  await nav.getByRole("link", { name: /Regler/ }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Regler" })).toBeVisible();
  await go(page, "#/series/41");
  await expect(page.getByRole("heading", { level: 1, name: "Sammenlign bildene i serien" })).toBeVisible();
  await expect(nav.getByRole("link", { name: /Gjennomgang/ })).toHaveAttribute("aria-current", "page");
});

test("Hjem har oversiktspanelet; Innstillinger har tre nye seksjoner", async ({ page }) => {
  await scanned(page);
  await expect(page.getByRole("group", { name: "Oversikt" })).toBeVisible();
  await go(page, "#/settings");
  for (const name of ["Hvor mange forslag", "Ansikter og personer", "Beskyttelser"]) await expect(page.getByRole("group", { name })).toBeVisible();
});

test("Gjennomgang og Kortmodus viser serier og regler med begrunnelse fra detail", async ({ page }) => {
  await scanned(page);
  await go(page, "#/review");
  await expect(page.getByRole("link", { name: /Serier/ })).toBeVisible();
  await expect(page.getByRole("link", { name: /Regelforslag/ })).toBeVisible();
  await page.getByRole("link", { name: /Serier/ }).click();
  await expect(page.getByText("Nesten like bilder tatt tett i tid")).toBeVisible();
  await expect(page.getByRole("link", { name: "Sammenlign serien" }).first()).toHaveAttribute("href", /#\/series\/4[12]$/);
  await go(page, "#/review/rule");
  await expect(page.getByText("Regler foreslår bare").first()).toBeVisible();
  await expect(page.getByText(/Regel: Gamle skjermbilder/).first()).toBeVisible();
  await go(page, "#/cards/series");
  await expect(page.getByText(/Bilde 2 av 4 i serien/).first()).toBeVisible();
});

for (const scheme of ["light", "dark"] as const) {
  test(`axe på nye flater ${scheme}`, async ({ page }) => {
    test.setTimeout(90_000);
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
    await scanned(page);
    await axe(page, "hjem med oversiktspanel");
    for (const hash of ["#/people", "#/rules", "#/series/41", "#/settings", "#/review/series", "#/review/rule", "#/cards/series"]) {
      await go(page, hash);
      await axe(page, hash);
    }
  });
}

test("ProtectToggle: aria-pressed, tastatur og navn som ikke endres", async ({ page }) => {
  await page.goto("/?gallery");
  const btn = page.getByRole("button", { name: "Beskytt", exact: true });
  await expect(btn).toHaveAttribute("aria-pressed", "false");
  await btn.focus();
  await page.keyboard.press("Space");
  await expect(btn).toHaveAttribute("aria-pressed", "true");
  await expect(btn).toContainText("På");
  await page.keyboard.press("Enter");
  await expect(btn).toHaveAttribute("aria-pressed", "false");
  await expect(btn).toContainText("Av");
  await expect(btn).toHaveAccessibleDescription(/beskyttede bilder krever alltid din egen bekreftelse/);
  await axe(page, "ProtectToggle", ".v2");
});
