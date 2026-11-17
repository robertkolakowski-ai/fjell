// Diagnose i Innstillinger (docs/v1/DIAGNOSE.md) mot MockBridge: tydelig om hva som er og ikke er med, forhåndsvisning før lagring,
// ingen sti i rapporten, ingen mappenavn som standard. Axe i lys og mørk, og pseudo-lokalet kuttes ikke.
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

test("diagnose sier hva som er og ikke er med, og viser rapporten før lagring", async ({ page }) => {
  await open(page, "#/settings");
  const g = page.getByRole("group", { name: "Diagnose" });
  await expect(g.getByText("Ikke med:")).toBeVisible();
  await expect(g.getByText("bilder, stier, filnavn, mappenavn", { exact: false })).toBeVisible();
  await expect(g.getByText("Mappenavn er ikke med", { exact: false })).toBeVisible();
  await expect(g.getByText("Dette er nøyaktig teksten som lagres", { exact: false })).toBeVisible();
  const text = await page.getByLabel("Slik ser rapporten ut").inputValue();
  expect(text).not.toMatch(/[\\/:]/);
  expect(text).not.toContain("folder.");
  // Ingen avkrysning for mappenavn i denne utgaven: de er alltid av.
  await expect(g.getByRole("checkbox")).toHaveCount(0);
});

test("Lagre rapport bekrefter, og ingenting sendes", async ({ page }) => {
  const requests: string[] = [];
  page.on("request", (r) => {
    if (!r.url().startsWith("http://localhost")) requests.push(r.url());
  });
  await open(page, "#/settings");
  await page.getByRole("button", { name: "Lagre rapport i diagnosemappen" }).click();
  await expect(page.locator("p").filter({ hasText: "Rapporten er lagret i Fjells diagnosemappe" })).toBeVisible();
  await expect(page.getByText("Fjell sender ingenting og åpner ikke e-post", { exact: false })).toBeVisible();
  expect(requests).toEqual([]);
});

for (const scheme of ["light", "dark"] as const) {
  test(`diagnose har ingen axe-brudd (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page, "#/settings");
    await axe(page, scheme);
  });
}

test("diagnose kuttes ikke i pseudo-lokalet og på smal skjerm", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await open(page, "#/settings", "pseudo");
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  expect(overflow).toBe(false);
  await expect(page.locator("#diag-text")).toBeVisible();
});
