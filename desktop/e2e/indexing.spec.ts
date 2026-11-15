// Indeksering (docs/v1/INDEKSERING.md): rolig fremdrift med Pause/Fortsett som knapper, tilstandsendringer i aria-live,
// gjennomgang er ikke blokkert mens det indekseres, og beskjed om at forslag oppdateres underveis. Mot MockBridge
// (`mockScanPolls` holder skanningen i gang). Axe i lys og mørk, bred og smal; pseudo-lokalet kuttes ikke; Pause/Fortsett < 100 ms.
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];
const RUNNING = "&mockScanPolls=100000";

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

/** Starter skanning på Hjem og går til Innstillinger (der fremdriften har knapper). */
async function startAndOpenSettings(page: Page, extra = "", lang: "nb" | "en" | "pseudo" = "nb"): Promise<void> {
  await open(page, "#/", lang, 0, RUNNING + extra);
  await page.getByRole("button", { name: lang === "en" ? "Start scan" : "Start skanning" }).first().click();
  await go(page, "#/settings");
}

const group = (page: Page) => page.getByRole("group", { name: "Fremdrift i indeksering" });

test("fremdrift har tall, fremdriftslinje og knapper, og sier at forslag oppdateres underveis", async ({ page }) => {
  await startAndOpenSettings(page);
  const g = group(page);
  await expect(g).toBeVisible();
  await expect(g.getByText("Indekserer …")).toBeVisible();
  await expect(g.getByRole("progressbar", { name: "Fremdrift i indeksering" })).toBeVisible();
  await expect(g.getByText(/\d.* av \d/)).toBeVisible();
  await expect(g.getByText(/igjen/).first()).toBeVisible();
  await expect(g.getByText("Forslagene oppdateres underveis.", { exact: false })).toBeVisible();
  await expect(g.getByRole("button", { name: "Pause" })).toBeVisible();
  await expect(g.getByRole("button", { name: "Avbryt" })).toBeVisible();
  // Rolig tekst: ingen utropstegn eller feilord.
  expect(await g.innerText()).not.toMatch(/!|feil|advarsel|mislyktes/i);
});

test("Pause og Fortsett er knapper, og endringen kunngjøres i aria-live (én gang per endring)", async ({ page }) => {
  await startAndOpenSettings(page);
  const g = group(page);
  await g.getByRole("button", { name: "Pause" }).click();
  await expect(g.getByText("Satt på pause.")).toBeVisible();
  await expect(g.getByRole("button", { name: "Fortsett" })).toBeVisible();
  await expect(page.locator("[role=status]").filter({ hasText: "Satt på pause." })).toHaveCount(1);
  await g.getByRole("button", { name: "Fortsett" }).click();
  await expect(g.getByText("Indekserer …")).toBeVisible();
  await expect(page.locator("[role=status]").filter({ hasText: "Indekserer …" })).toHaveCount(1);
  await g.getByRole("button", { name: "Avbryt" }).click();
  await expect(page.getByRole("group", { name: "Fremdrift i indeksering" })).toHaveCount(0);
  await expect(page.locator("[role=status]").filter({ hasText: "Indeksering er ferdig." })).toHaveCount(1);
});

test("på batteri: rolig forklaring, og Fortsett finnes som knapp", async ({ page }) => {
  await startAndOpenSettings(page, "&mockBattery=1");
  const g = group(page);
  await expect(g.getByText("Satt på pause fordi PC-en går på batteri.")).toBeVisible({ timeout: 10_000 });
  await expect(g.getByText("Fortsetter av seg selv når PC-en er koblet til strøm", { exact: false })).toBeVisible();
  await expect(g.getByRole("button", { name: "Fortsett" })).toBeVisible();
  await expect(g.getByRole("button", { name: "Pause" })).toHaveCount(0);
});

test("gjennomgang er ikke blokkert mens det indekseres", async ({ page }) => {
  await open(page, "#/", "nb", 0, RUNNING);
  await page.getByRole("button", { name: "Start skanning" }).click();
  await expect(page.getByRole("button", { name: "Pause" }).first()).toBeVisible();
  await go(page, "#/cards/exact_dup");
  await expect(page.locator("h1").first()).toBeVisible();
  await expect(page.getByRole("button", { name: /Beholdes|Behold|Slett|Legg i/ }).first()).toBeEnabled();
});

test("Pause og Fortsett svarer under 100 ms (p95) under pågående indeksering", async ({ page }) => {
  await startAndOpenSettings(page);
  await group(page).waitFor();
  const sel = `[...document.querySelectorAll('[aria-label="Fremdrift i indeksering"] button')]`;
  const ms: number[] = [];
  for (let i = 0; i < 8; i++) {
    const pause = i % 2 === 0;
    const label = pause ? "Pause" : "Fortsett";
    const text = pause ? "Satt på pause" : "Indekserer";
    ms.push(await page.evaluate(
      ({ sel, label, text }) => new Promise<number>((resolve, reject) => {
        const root = () => document.querySelector('[aria-label="Fremdrift i indeksering"]');
        const fail = setTimeout(() => reject(new Error("tidsavbrudd")), 3000);
        const t0 = performance.now();
        const mo = new MutationObserver(() => {
          if (root()?.textContent?.includes(text)) {
            clearTimeout(fail); mo.disconnect();
            requestAnimationFrame(() => resolve(performance.now() - t0));
          }
        });
        mo.observe(document.body, { subtree: true, childList: true, characterData: true });
        (new Function(`return ${sel}.find((b) => b.textContent.trim() === ${JSON.stringify(label)})`)() as HTMLButtonElement).click();
      }),
      { sel, label, text },
    ));
  }
  const sorted = [...ms].sort((a, b) => a - b);
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
  expect(p95, `målinger: ${ms.map((m) => m.toFixed(0)).join(", ")} ms`).toBeLessThan(100);
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of [["bred", { width: 1280, height: 800 }], ["smal", { width: 360, height: 740 }]] as const) {
    test(`axe indeksering ${scheme} ${vp}: 0 brudd (kjører og pauset)`, async ({ page }) => {
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await startAndOpenSettings(page);
      await group(page).waitFor();
      await axe(page, `kjører ${scheme} ${vp}`);
      await group(page).getByRole("button", { name: "Pause" }).click();
      await expect(group(page).getByText("Satt på pause.")).toBeVisible();
      await axe(page, `pauset ${scheme} ${vp}`);
    });
  }
}

test("pseudo-lokale: ingen horisontal rulling i fremdriftsraden (pågår og pauset)", async ({ page }) => {
  for (const w of [1280, 360]) {
    await page.setViewportSize({ width: w, height: 800 });
    await open(page, "#/onboarding", "pseudo", 0, RUNNING);
    // Pseudo-lokalet har forvrengt tekst: strukturelle velgere (som i tour.ts).
    await page.locator(".onb > .row button").first().click();
    await page.locator(".onb > .row button").last().click();
    await go(page, "#/settings");
    await page.locator('[aria-label]').filter({ has: page.locator("progress") }).first().waitFor();
    for (const pass of ["kjører", "pauset"]) {
      const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(over, `${w}px ${pass}`).toBeLessThanOrEqual(0);
      if (pass === "kjører") await page.locator("progress").locator("xpath=ancestor::div[@role='group']").getByRole("button").first().click();
      await page.waitForTimeout(150);
    }
  }
});
