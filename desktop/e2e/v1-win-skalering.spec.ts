// V1 ved Windows-skalering 200 % (emulert: CSS-viewport = fysisk oppløsning / 2, deviceScaleFactor 2, rot-skrift 200 %), på minstevindu,
// med pseudo-lokale (nb +30 til 40 %) og med WCAG 1.4.12 tekstavstand. Gjelder alle V1-skjermer og V1-dialogene.
// Bare Chromium her; ekte Windows-skalering (150 %, 175 %, 200 % og flere skjermer) står i docs/v1/WINDOWS-V1.md del 5.
import { expect, test, type Page } from "@playwright/test";
import { go, type Lang } from "./helpers";
import { layoutProblemer, scanned, V1_SKJERMER } from "./v1-win-felles";

const VINDU = [
  { navn: "200 % på 1920x1080", w: 960, h: 540, dpr: 2, tekst: 200 },
  { navn: "200 % minstevindu (Tauri 480x520)", w: 480, h: 520, dpr: 2, tekst: 200 },
] as const;

/** Setter rot-skriftstørrelsen som «Gjør tekst større» gjør, før siden lastes. */
async function skaler(page: Page, pct: number): Promise<void> {
  await page.addInitScript((p) => {
    document.addEventListener("DOMContentLoaded", () => { document.documentElement.style.fontSize = `${p}%`; });
  }, pct);
}

/** Alle synlige kontroller ligger innenfor vinduet i bredden og er minst 24x24 CSS-px (WCAG 2.5.8), unntatt lenker i løpende tekst. */
async function kontrollstorrelse(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    for (const el of Array.from(document.querySelectorAll<HTMLElement>("button, a[href], select, input:not([type=hidden])"))) {
      if (!el.checkVisibility({ checkVisibilityCSS: true }) || el.closest(".sr-only") || el.closest("dialog:not([open])")) continue;
      const r = el.getBoundingClientRect();
      const navn = `${el.tagName.toLowerCase()} «${(el.getAttribute("aria-label") ?? el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 28)}»`;
      if (r.left < -1 || r.right > innerWidth + 1) out.push(`${navn} utenfor vinduet (${Math.round(r.left)}..${Math.round(r.right)} av ${innerWidth})`);
      const iTekst = el instanceof HTMLAnchorElement && !!el.closest("p, li") && getComputedStyle(el).display === "inline";
      const avkrysning = el instanceof HTMLInputElement && (el.type === "checkbox" || el.type === "radio");
      // Avkrysningsbokser og radioer har egen klikkflate via label; måles på label.
      const flate = avkrysning && el.labels?.[0] ? el.labels[0].getBoundingClientRect() : r;
      if (!iTekst && (flate.width < 24 || flate.height < 24)) out.push(`${navn} for liten: ${Math.round(flate.width)}x${Math.round(flate.height)}`);
    }
    return out.slice(0, 8);
  });
}

for (const v of VINDU) {
  test(`${v.navn}: V1-skjermene uten avkutting, horisontal rulling eller for små kontroller (nb)`, async ({ browser }) => {
    test.setTimeout(150_000);
    const ctx = await browser.newContext({ viewport: { width: v.w, height: v.h }, deviceScaleFactor: v.dpr });
    const page = await ctx.newPage();
    await skaler(page, v.tekst);
    await scanned(page, "#/");
    expect(await page.evaluate(() => devicePixelRatio)).toBe(v.dpr);
    expect(await page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).fontSize))).toBeCloseTo(16 * (v.tekst / 100), 1);
    const funn: string[] = [];
    for (const s of V1_SKJERMER) {
      await go(page, s.hash);
      await page.waitForTimeout(120);
      funn.push(...(await layoutProblemer(page)).map((f) => `${s.navn}: ${f}`));
      funn.push(...(await kontrollstorrelse(page)).map((f) => `${s.navn}: ${f}`));
    }
    expect(funn, funn.join("\n")).toEqual([]);
    await ctx.close();
  });
}

test("200 % minstevindu med pseudo-lokale: V1-skjermene uten avkutting", async ({ browser }) => {
  test.setTimeout(150_000);
  const ctx = await browser.newContext({ viewport: { width: 480, height: 520 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await skaler(page, 200);
  await scanned(page, "#/", "pseudo" as Lang);
  const funn: string[] = [];
  for (const s of V1_SKJERMER) {
    await go(page, s.hash);
    await page.waitForTimeout(120);
    funn.push(...(await layoutProblemer(page)).map((f) => `${s.navn}: ${f}`));
  }
  expect(funn, funn.join("\n")).toEqual([]);
  await ctx.close();
});

test("200 % minstevindu: V1-dialogene ligger innenfor vinduet og knappene kan nås", async ({ browser }) => {
  test.setTimeout(120_000);
  const ctx = await browser.newContext({ viewport: { width: 480, height: 520 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await skaler(page, 200);
  await scanned(page, "#/rules");
  const funn: string[] = [];
  const sjekk = async (hva: string) => {
    const dlg = page.getByRole("dialog");
    await expect(dlg).toBeVisible();
    const r = await dlg.evaluate((d) => {
      const b = d.getBoundingClientRect();
      const knapper = Array.from(d.querySelectorAll<HTMLElement>("button")).filter((k) => k.checkVisibility({ checkVisibilityCSS: true }));
      // Innhold som ikke får plass skal rulle inne i dialogen; da kan knappen scrolles til og er nåbar.
      const rullbar = d.scrollHeight > d.clientHeight + 1 || Array.from(d.querySelectorAll("*")).some((e) => { const o = getComputedStyle(e).overflowY; return (o === "auto" || o === "scroll") && e.scrollHeight > e.clientHeight; });
      return { l: b.left, r: b.right, t: b.top, b: b.bottom, w: innerWidth, h: innerHeight, antall: knapper.length, rullbar };
    });
    if (r.l < -1 || r.r > r.w + 1) funn.push(`${hva}: dialogen går utenfor i bredden (${Math.round(r.l)}..${Math.round(r.r)} av ${r.w})`);
    if ((r.t < -1 || r.b > r.h + 1) && !r.rullbar) funn.push(`${hva}: dialogen går utenfor i høyden uten å kunne rulle (${Math.round(r.t)}..${Math.round(r.b)} av ${r.h})`);
    // Siste knapp må kunne gjøres synlig og klikkbar (Playwright feiler hvis den er skjult bak noe).
    const siste = dlg.getByRole("button").last();
    await siste.scrollIntoViewIfNeeded();
    const boks = await siste.boundingBox();
    if (!boks || boks.x < -1 || boks.x + boks.width > r.w + 1) funn.push(`${hva}: siste knapp utenfor vinduet`);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toBeHidden();
  };
  await page.getByRole("button", { name: "Ny regel" }).first().click();
  await sjekk("Ny regel");
  await page.getByRole("button", { name: "Rediger regelen Store filer" }).click();
  await sjekk("Rediger regel");
  await go(page, "#/people");
  await page.getByRole("button", { name: "Slå på ansiktsindeks" }).click();
  await sjekk("Samtykke til ansiktsindeks");
  await go(page, "#/settings");
  await page.getByRole("button", { name: "Beskytt en mappe" }).click();
  await sjekk("Beskytt en mappe");
  expect(funn, funn.join("\n")).toEqual([]);
  await ctx.close();
});

test("WCAG 1.4.12 tekstavstand: V1-skjermene mister ingen tekst ved økt linje-, bokstav-, ord- og avsnittsavstand", async ({ browser }) => {
  test.setTimeout(120_000);
  const ctx = await browser.newContext({ viewport: { width: 960, height: 540 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await skaler(page, 200);
  await scanned(page, "#/");
  await page.addStyleTag({ content: "* { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; } p { margin-bottom: 2em !important; }" });
  const funn: string[] = [];
  for (const s of V1_SKJERMER) {
    await go(page, s.hash);
    await page.waitForTimeout(120);
    funn.push(...(await layoutProblemer(page)).map((f) => `${s.navn}: ${f}`));
  }
  expect(funn, funn.join("\n")).toEqual([]);
  await ctx.close();
});

test("selvtest: layoutkontrollen finner horisontal rulling (testen er ikke tom)", async ({ page }) => {
  await scanned(page, "#/");
  await page.addStyleTag({ content: "main { min-width: 3000px !important; }" });
  const funn = await layoutProblemer(page);
  expect(funn.join()).toContain("horisontal rulling");
});

test("selvtest: størrelseskontrollen finner for små kontroller (testen er ikke tom)", async ({ page }) => {
  await scanned(page, "#/rules");
  await page.addStyleTag({ content: "button { min-width: 0 !important; min-height: 0 !important; width: 10px !important; height: 10px !important; padding: 0 !important; overflow: hidden; }" });
  const funn = await kontrollstorrelse(page);
  expect(funn.join()).toContain("for liten");
});
