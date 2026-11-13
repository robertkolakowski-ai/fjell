// Miniatyrer (M12): bilder i Gruppe, Kortmodus og Papirkurv, forklart feil per kode, axe, høykontrast, pseudo og ytelse.
// Skjermbilder til docs/screens/m10/ med SHOTS=1.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { go, open, seedScan, timed, type Lang } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];
/** Gruppe 1: keeper 1001 + kort 1 og 2. Uskarpe: kort 13 til 18. */
const ERRS = "&mockThumbErr=1001:unavailable,1:cloud_placeholder,2:corrupt,13:not_found,14:no_library,15:unavailable";
const TEXT = {
  nb: { unavailable: "Kan ikke vise dette formatet ennå", cloud_placeholder: "Ikke lastet ned fra skyen", corrupt: "Filen ser ødelagt ut", not_found: "Finner ikke filen", no_library: "Velg en mappe først" },
  en: { unavailable: "Can't show this format yet", cloud_placeholder: "Not downloaded from the cloud", corrupt: "The file looks damaged", not_found: "Can't find the file", no_library: "Choose a folder first" },
} as const;

async function axe(page: Page, where: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const res = await page.evaluate(async (tags) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; nodes: { target: unknown[] }[] }) => ({ id: v.id, nodes: v.nodes.slice(0, 3).map((n) => n.target.join(" ")) }));
  }, tags());
  expect(res, `axe-brudd: ${where}`).toEqual([]);
  function tags() { return TAGS; }
}

async function start(page: Page, hash: string, lang: Lang = "nb", extra = ""): Promise<void> {
  await open(page, "#/onboarding", lang, 0, extra);
  if (lang === "pseudo") {
    // Pseudo har andre ord: primærknappene står likevel på samme sted.
    await page.locator(".onb > .row .btn.primary").click();
    await page.locator(".onb > .row .btn.primary").click();
    await page.locator(".onb .btn.primary").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 });
  } else await seedScan(page);
  await go(page, hash);
}

/** Alle <img> i valgt område er ferdig lastet og har faktisk størrelse. */
/** Miniatyrer laster først når de nærmer seg visning: rull gjennom siden så alt er lastet. */
async function scrollAll(page: Page, sel: string): Promise<void> {
  for (const el of await page.locator(sel).all()) await el.scrollIntoViewIfNeeded();
  await page.evaluate(() => window.scrollTo(0, 0));
}

async function imagesLoaded(page: Page, sel: string, min: number): Promise<void> {
  await expect.poll(() => page.evaluate((s) => [...document.querySelectorAll<HTMLImageElement>(s)].filter((i) => i.complete && i.naturalWidth > 0).length, sel)).toBeGreaterThanOrEqual(min);
}

test("Gruppe: sammenligningsbildene vises store med riktig sideforhold", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await start(page, "#/group/1");
  await imagesLoaded(page, ".items img", 3);
  const boxes = await page.locator(".items .img").evaluateAll((els) => els.map((e) => e.getBoundingClientRect().width));
  expect(Math.min(...boxes)).toBeGreaterThan(180);
  const r = await page.locator(".items img").first().evaluate((i: HTMLImageElement) => ({ w: i.naturalWidth, h: i.naturalHeight, fit: getComputedStyle(i).objectFit }));
  expect(r.w / r.h).toBeCloseTo(256 / 192, 1);
  expect(r.fit).toBe("contain");
  await expect(page.getByRole("img", { name: "Bilde 1 av 3" })).toBeVisible();
});

test("Kortmodus: stort kort viser bildet", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await start(page, "#/cards");
  await imagesLoaded(page, ".card.front img", 1);
  const h = await page.locator(".card.front img").evaluate((i) => i.getBoundingClientRect().height);
  expect(h).toBeGreaterThan(200);
});

test("Gjennomgang: bildeflis og grupperader viser miniatyr", async ({ page }) => {
  await start(page, "#/review/blurry");
  await imagesLoaded(page, ".tile img", 6);
  await go(page, "#/review/exact_dup");
  await imagesLoaded(page, ".thumbs img", 2);
});

test("Papirkurv: rader viser miniatyr etter flytting", async ({ page }) => {
  await start(page, "#/review/blurry");
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt 6/ }).click();
  await expect(page.getByText("6 flyttet")).toBeVisible();
  await go(page, "#/trash");
  await imagesLoaded(page, ".row img", 6);
});

for (const lang of ["nb", "en"] as const) {
  test(`feiltilstander har forklaring per kode (${lang})`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await start(page, "#/group/1", lang, ERRS);
    const t = TEXT[lang];
    await expect(page.getByText(t.unavailable)).toBeVisible();
    await expect(page.getByText(t.cloud_placeholder)).toBeVisible();
    await expect(page.getByText(t.corrupt)).toBeVisible();
    await go(page, "#/review/blurry");
    // Flisene (knapper) tegner forklaringen som generert innhold og har den i navnet.
    await expect(page.locator(".tile").nth(0)).toHaveAccessibleName(new RegExp(t.not_found));
    await expect(page.locator(".tile").nth(1)).toHaveAccessibleName(new RegExp(t.no_library));
    await expect(page.locator(".tile .txt").nth(0)).toBeVisible();
    expect(await page.locator(".tile .txt").nth(0).evaluate((e) => getComputedStyle(e, "::after").content)).toContain(t.not_found);
    // Forklaringen står også i flisens navn (skjermleser), og feil hindrer ikke valg.
    const tile = page.locator(".tile").nth(0);
    await expect(tile).toHaveAccessibleName(new RegExp(t.not_found));
    await tile.click();
    await expect(tile).toHaveAttribute("aria-pressed", "true");
  });
}

test("feil i små rader (Papirkurv) får forklaring som navn", async ({ page }) => {
  await start(page, "#/review/blurry", "nb", ERRS);
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt 6/ }).click();
  await expect(page.getByText("6 flyttet")).toBeVisible();
  await go(page, "#/trash");
  await expect(page.locator(".row .err[role=img][aria-label='Finner ikke filen']")).toHaveCount(1);
  await expect(page.locator(".row .err[aria-label='Velg en mappe først']")).toHaveCount(1);
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of [["bred", { width: 1280, height: 800 }], ["smal", { width: 360, height: 740 }]] as const) {
    test(`axe med bilder og feil: ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await start(page, "#/group/1", "nb", ERRS);
      await scrollAll(page, ".items .item");
      await expect(page.getByText(TEXT.nb.corrupt)).toBeVisible();
      await axe(page, "gruppe med feil");
      await go(page, "#/review/blurry");
      await scrollAll(page, ".tile");
      await imagesLoaded(page, ".tile img", 3);
      await axe(page, "gjennomgang");
      await go(page, "#/cards");
      await expect(page.locator(".card.front .thumb:not([data-state=idle]):not([data-state=loading])")).toBeVisible(); // kort 1 har feil i denne økten
      await axe(page, "kortmodus");
      await go(page, "#/review/blurry");
      await page.getByRole("button", { name: "Velg alle" }).click();
      await page.getByRole("button", { name: /^Flytt \d/ }).click();
      await expect(page.getByText(/\d flyttet/)).toBeVisible();
      await go(page, "#/trash");
      await page.locator(".row .thumb").first().waitFor();
      await axe(page, "papirkurv");
    });
  }
}

test("skeleton under lasting: aksessibel og uten animasjon ved redusert bevegelse", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await open(page, "#/onboarding", "nb", 0, "&mockThumbDelay=1500");
  await seedScan(page);
  await go(page, "#/group/1");
  const skel = page.locator(".items .skel").first();
  await expect(skel).toBeVisible();
  expect(await skel.evaluate((e) => getComputedStyle(e).animationName)).toBe("none");
  await axe(page, "skeleton");
});

test("høykontrast (forced-colors): bilder, skeleton og feil synlige", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await start(page, "#/group/1", "nb", ERRS);
  await expect(page.getByText(TEXT.nb.corrupt)).toBeVisible();
  const err = page.locator(".err").first();
  const [bw, style] = await err.evaluate((e) => [getComputedStyle(e).borderTopWidth, getComputedStyle(e).borderTopStyle]);
  expect(parseFloat(bw)).toBeGreaterThan(0);
  expect(style).not.toBe("none");
  await go(page, "#/review/blurry");
  await imagesLoaded(page, ".tile img", 3);
  await axe(page, "høykontrast");
});

test("pseudo-lokale: forklaringer kuttes ikke (bred og smal)", async ({ page }) => {
  for (const size of [{ width: 1280, height: 800 }, { width: 360, height: 740 }]) {
    await page.setViewportSize(size);
    await start(page, "#/group/1", "pseudo", ERRS);
    await page.locator(".items .err .txt").first().waitFor();
    for (const hash of ["#/group/1", "#/review/blurry"]) {
      await go(page, hash);
      await page.locator(".err .txt").first().waitFor();
      const clipped = await page.evaluate(() => [...document.querySelectorAll<HTMLElement>(".err .txt")].filter((t) => {
        const box = t.closest<HTMLElement>(".thumb")!.getBoundingClientRect();
        const r = t.getBoundingClientRect();
        return r.top < box.top - 0.5 || r.bottom > box.bottom + 0.5 || r.left < box.left - 0.5 || r.right > box.right + 0.5 || t.scrollWidth > t.clientWidth + 1;
      }).map((t) => t.textContent));
      expect(clipped, `${size.width}px ${hash}`).toEqual([]);
    }
  }
});

test("ytelse: handlinger p95 < 100 ms med bilder", async ({ page }) => {
  test.setTimeout(120_000);
  await page.setViewportSize({ width: 1280, height: 800 });
  await start(page, "#/review/blurry");
  await imagesLoaded(page, ".tile img", 6);
  const xs: number[] = [];
  for (let r = 0; r < 6; r++) {
    for (let i = 0; i < 6; i++) {
      xs.push(await timed(page, `document.querySelectorAll('.tile')[${i}].click()`, `document.querySelectorAll('.tile')[${i}].getAttribute('aria-pressed')===${r % 2 === 0 ? "'true'" : "'false'"}`));
    }
  }
  await go(page, "#/cards");
  await imagesLoaded(page, ".card.front img", 1);
  for (let i = 0; i < 6; i++) {
    const before = await page.locator(".card.front img").getAttribute("src");
    xs.push(await timed(page, `document.querySelector('.deck').dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowLeft',bubbles:true}))`, `document.querySelector('.card.front')!==null && (document.querySelector('.card.front img')?.getAttribute('src')!==${JSON.stringify(before)} || !document.querySelector('.card.front img'))`));
  }
  xs.sort((a, b) => a - b);
  const p95 = xs[Math.min(xs.length - 1, Math.ceil(0.95 * xs.length) - 1)];
  expect(p95, `p95 ${p95} ms`).toBeLessThan(100);
});

test.describe("skjermbilder", () => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  test.use({ deviceScaleFactor: 1 });
  const OUT = process.env.SHOTS_DIR ?? "../docs/screens/m10";
  const shot = async (page: Page, name: string) => {
    mkdirSync(OUT, { recursive: true });
    await page.waitForTimeout(300);
    await page.screenshot({ path: `${OUT}/${name}.jpg`, type: "jpeg", quality: 55 });
  };
  test("miniatyrer", async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
    await start(page, "#/review/exact_dup");
    await go(page, "#/group/3");
    await imagesLoaded(page, ".items img", 3);
    await shot(page, "miniatyr-gruppe-nb-light-1280");
    await go(page, "#/cards");
    await imagesLoaded(page, ".card.front img", 1);
    await shot(page, "miniatyr-kortmodus-nb-light-1280");
    await go(page, "#/review/blurry");
    await page.getByRole("button", { name: "Velg alle" }).click();
    await page.getByRole("button", { name: /^Flytt \d/ }).click();
    await expect(page.getByText(/\d flyttet/)).toBeVisible();
    await go(page, "#/trash");
    await imagesLoaded(page, ".row img", 3);
    await shot(page, "miniatyr-papirkurv-nb-light-1280");
    await start(page, "#/group/1", "nb", ERRS);
    await expect(page.getByText(TEXT.nb.corrupt)).toBeVisible();
    await shot(page, "miniatyr-feil-nb-light-1280");
  });
});
