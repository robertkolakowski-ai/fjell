// Oversikt (dashboard) på Hjem mot MockBridge: tall, lenker, «Neste beste grep» (knapp, aldri automatisk), «Tømt» uten påstand om
// frigjort plass, tabell for tømt per måned, tomtilstand, axe 0 brudd (lys/mørk × bred/smal), pseudo uten avkutting.
// Skjermbilder (nb/en, lys/mørk) med SHOTS=1 (SHOTS_DIR, standard ../docs/screens/v1).
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open, seedScan, type Lang } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

async function axe(page: Page, where: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const res = await page.evaluate(async (tags) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; nodes: { target: unknown[] }[] }) => ({ id: v.id, nodes: v.nodes.slice(0, 3).map((n) => n.target.join(" ")) }));
  }, TAGS);
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

/** Panelet monteres av Hjem i `.dash`; samme velger i alle språk (pseudo har forvrengt tekst). */
const panel = (page: Page) => page.locator(".dash");

async function scanned(page: Page, lang: Lang = "nb"): Promise<void> {
  if (lang === "pseudo") {
    // Strukturelle velgere som i tour.ts: pseudo-teksten kan ikke matches på navn.
    await open(page, "#/onboarding", lang);
    await page.locator(".onb > .row button").first().click();
    await page.locator(".onb > .row button").last().click();
    await page.locator("h1").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 });
  } else {
    await open(page, "#/", lang);
    await seedScan(page);
  }
  await go(page, "#/");
  await panel(page).locator("ul.tiles").first().waitFor();
}

/** Tøm papirkurven for godt (eneste vei der kjernen teller «Tømt» i mocken). */
async function emptyPermanently(page: Page): Promise<void> {
  await go(page, "#/trash");
  await page.getByRole("button", { name: "Tøm papirkurv …" }).click();
  const dlg = page.getByRole("dialog", { name: "Tøm papirkurven" });
  await dlg.getByRole("radio", { name: "Slett permanent" }).check();
  await dlg.getByRole("button", { name: "Fortsett" }).click();
  const perm = page.getByRole("dialog", { name: "Dette kan ikke angres" });
  await perm.getByRole("checkbox").check();
  await perm.getByRole("button", { name: /^Slett \d+ permanent/ }).click();
  await expect(page.locator(".done").getByText(/er permanent slettet/)).toBeVisible();
}

async function trashBlurry(page: Page): Promise<void> {
  await go(page, "#/review/blurry");
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt \d+/ }).click();
  await expect(page.getByText(/flyttet/).first()).toBeVisible();
}

test("tom oversikt før skann, så tall etter skann", async ({ page }) => {
  await open(page, "#/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  // Før skann viser Hjem bare «Start skanning»: panelet er ikke montert, ingen feil.
  await expect(page.getByRole("alert")).toHaveCount(0);
  await scanned(page);
  const p = panel(page);
  await expect(p.getByRole("heading", { level: 3, name: "Kan ryddes per type" })).toBeVisible();
  for (const name of ["Brukt plass", "Kan ryddes", "I papirkurven", "Tømt"]) await expect(p.getByText(name, { exact: true }).first()).toBeVisible();
  await expect(p.getByText(/Ingenting er tømt ennå/).first()).toBeVisible();
  // Én flis per forslagstype, alle seks, med lenke til Gjennomgang.
  for (const [name, hash] of [
    ["Eksakte duplikater", "exact_dup"], ["Like bilder", "visual_dup"], ["Uskarpe", "blurry"], ["Skjermbilder", "screenshot"], ["Serier", "series"], ["Regelforslag", "rule"],
  ] as const) {
    await expect(p.getByRole("link", { name: new RegExp(`^${name}`) }).first()).toHaveAttribute("href", new RegExp(`#/review/${hash}$`));
  }
  await expect(p.getByRole("img", { name: /av \d+ forslag gjennomgått/ })).toBeVisible();
});

test("«Neste beste grep» er en knapp som åpner Gjennomgang, og skjer aldri av seg selv", async ({ page }) => {
  await scanned(page);
  const p = panel(page);
  await page.waitForTimeout(500);
  expect(new URL(page.url()).hash).toBe("#/");
  const btn = p.getByRole("button", { name: "Gå gjennom forslagene" });
  await expect(btn).toBeVisible();
  await btn.focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#\/review\/[a-z_]+$/);
  await expect(page.getByRole("heading", { level: 1 })).toBeFocused();
});

test("papirkurv og «Tømt»: ingen påstand om frigjort plass, tabell per måned, lenke til papirkurven", async ({ page }) => {
  await scanned(page);
  await trashBlurry(page);
  await go(page, "#/");
  const p = panel(page);
  await p.getByRole("heading", { level: 3, name: "Neste steg" }).waitFor();
  await expect(p.getByRole("link", { name: /^I papirkurven/ })).toHaveAttribute("href", "#/trash");
  await expect(p.getByText(/Ingenting er tømt ennå/).first()).toBeVisible();
  await expect(p.getByRole("table")).toHaveCount(0);
  expect(await p.innerText()).not.toMatch(/frigjort(?!s)/i); // aldri påstand om frigjort diskplass

  // Etter tømming får «Tømt» tall og tabell; forklaringen sier at plassen først frigjøres når Windows Papirkurv tømmes.
  await emptyPermanently(page);
  await go(page, "#/");
  await p.getByRole("heading", { level: 3, name: "Neste steg" }).waitFor();
  const table = p.getByRole("table");
  await expect(table).toBeVisible();
  await expect(table.getByRole("columnheader")).toHaveText(["Måned", "Bilder", "Størrelse"]);
  await expect(table.getByRole("rowheader")).toHaveCount(1);
  await expect(p.getByText(/plassen frigjøres først når du tømmer den/)).toBeVisible();
});

test("små tellere og største mapper", async ({ page }) => {
  await scanned(page);
  const p = panel(page);
  await expect(p.getByRole("link", { name: /^Beskyttede/ })).toHaveAttribute("href", "#/settings");
  await expect(p.getByRole("link", { name: /^Personer/ })).toHaveAttribute("href", "#/people");
  await expect(p.getByRole("link", { name: /^Regler/ })).toHaveAttribute("href", "#/rules");
  await expect(p.getByRole("heading", { level: 3, name: "Største mapper" })).toBeVisible();
  await expect(p.getByRole("listitem").filter({ hasText: /Familie/ })).toHaveCount(1);
});

const VIEWPORTS = { bred: { width: 1280, height: 900 }, smal: { width: 360, height: 740 } } as const;
for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`axe oversikt ${scheme} ${vp}`, async ({ page }) => {
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await scanned(page);
      await axe(page, "oversikt med forslag");
      await trashBlurry(page);
      await emptyPermanently(page);
      await go(page, "#/");
      await expect(panel(page).getByRole("table")).toBeVisible();
      await axe(page, "oversikt med tømt-tabell");
    });
  }
}

for (const lang of ["pseudo", "en"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`ingen avkutting i ${lang} (${vp})`, async ({ page }) => {
      await page.setViewportSize(size);
      await scanned(page, lang);
      const out = await page.evaluate(() => {
        const out: string[] = [];
        const de = document.documentElement;
        if (de.scrollWidth > de.clientWidth + 1) out.push(`horisontal rulling ${de.scrollWidth} > ${de.clientWidth}`);
        for (const el of document.querySelectorAll<HTMLElement>(".dash *")) {
          const cs = getComputedStyle(el);
          if ((cs.overflowX === "hidden" || cs.textOverflow === "ellipsis") && el.scrollWidth > el.clientWidth + 1) out.push(`avkuttet: ${el.tagName}.${el.className}`);
          if (el.matches("a, button") && el.scrollWidth > el.clientWidth + 1) out.push(`renner over: ${el.textContent}`);
        }
        return out;
      });
      expect(out).toEqual([]);
    });
  }
}

test("skjermbilder av oversikten", async ({ page }) => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  test.setTimeout(180_000);
  const dir = process.env.SHOTS_DIR ?? "../docs/screens/v1";
  await page.setViewportSize({ width: 1100, height: 900 });
  for (const lang of ["nb", "en"] as const) {
    for (const scheme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await scanned(page, lang);
      await panel(page).scrollIntoViewIfNeeded();
      await page.waitForTimeout(250);
      await panel(page).screenshot({ path: `${dir}/oversikt-${lang}-${scheme}.jpg`, type: "jpeg", quality: 60 });
    }
  }
});
