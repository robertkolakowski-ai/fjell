// Serie-sammenligning og forklarbare kort (gren `series`): flyten mot MockBridge, tastatur og knapper gir samme handling,
// ingen avkutting i pseudo-språk, og axe (lys/mørk × bred/smal, 0 brudd). Skjermbilder: SHOTS=1 (docs/screens/series/).
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open, type Lang } from "./helpers";

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

/** Åpner serien etter en skanning (mocken har da to serier og regelkort). */
async function scanned(page: Page, hash: string, lang: Lang = "nb"): Promise<void> {
  await open(page, "#/", lang);
  // Uavhengig av språk (pseudo har andre ord): knappene i onboarding finnes på samme sted.
  await go(page, "#/onboarding");
  await page.locator(".onb > .row .btn.primary").click();
  await page.locator(".onb > .row .btn.primary").click();
  await page.locator(".onb .btn.primary").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 });
  await go(page, hash);
}
const items = (page: Page) => page.locator("ul.items > li.item");

test("Gjennomgang -> Sammenlign serien: medlemmer side om side, behold-kandidat med hake og tekst, resten forhåndsmerket", async ({ page }) => {
  await scanned(page, "#/review/series");
  await page.getByRole("link", { name: "Sammenlign serien" }).first().click();
  await expect(page.getByRole("heading", { level: 1, name: "Sammenlign bildene i serien" })).toBeVisible();
  await expect(items(page)).toHaveCount(4);
  const best = items(page).first();
  await expect(best).toHaveClass(/best/);
  await expect(best.getByText("Foreslått å beholde")).toBeVisible();
  await expect(best.locator(".tick")).toBeVisible(); // hake i tillegg til teksten og rammen
  await expect(best.getByRole("checkbox")).toHaveCount(0);
  for (const i of [1, 2, 3]) await expect(items(page).nth(i).getByRole("checkbox", { name: "Flytt til papirkurv" })).toBeChecked();
  // Rangering, poeng og begrunnelse som tall og tekst.
  await expect(best.locator('[data-info="rank"]')).toHaveText("1");
  await expect(best.locator('[data-info="score"]')).toHaveText("0,92");
  await expect(best.locator('[data-info="reason"]')).toHaveText("Best i serien");
  await expect(items(page).nth(1).locator('[data-info="rank"]')).toHaveText("2");
  await expect(items(page).nth(1).getByText("Bilde 2 av 4 i serien").first()).toBeVisible();
  await expect(items(page).nth(1).getByText("Plass i serien")).toBeVisible();
  await expect(page.getByRole("button", { name: "Flytt 3 til papirkurv" })).toBeVisible();
});

test("flytt resten er ett angrebart valg", async ({ page }) => {
  await scanned(page, "#/series/41");
  await page.getByRole("button", { name: "Flytt 3 til papirkurv" }).click();
  await expect(page.getByText("Serien er ferdig. 3 bilder i papirkurven.")).toBeVisible();
  await expect(page.locator(".done")).toBeFocused();
  await page.getByRole("button", { name: "Angre" }).click();
  await expect(items(page)).toHaveCount(4);
  await expect(page.getByRole("button", { name: "Flytt 3 til papirkurv" })).toBeVisible();
});

test("avmerking endrer summen; Behold alle er også angrebart", async ({ page }) => {
  await scanned(page, "#/series/41");
  await items(page).nth(2).getByRole("checkbox").uncheck();
  await expect(page.getByRole("button", { name: "Flytt 2 til papirkurv" })).toBeVisible();
  await page.getByRole("button", { name: "Behold alle" }).click();
  await expect(page.getByText("Alle bildene i serien beholdes.")).toBeVisible();
  await page.getByRole("button", { name: "Angre" }).click();
  await expect(items(page)).toHaveCount(4);
});

test("Gjør dette til beste: knapp og tast gir samme resultat, og fokus havner på bildet", async ({ page }) => {
  await scanned(page, "#/series/41");
  const before = await items(page).first().getAttribute("data-asset");
  const third = items(page).nth(2);
  const pick = await third.getAttribute("data-asset");
  await third.getByRole("button", { name: "Gjør dette til beste" }).click();
  await expect(items(page).first()).toHaveAttribute("data-asset", pick!);
  await expect(items(page).first()).toHaveClass(/best/);
  await expect(items(page).first().getByText("Foreslått å beholde")).toBeVisible();
  await expect(page.locator(`[data-name="${pick}"]`)).toBeFocused();
  // Forhåndsmerkingen regnes på nytt: den gamle behold-kandidaten er nå merket for papirkurv.
  await expect(page.locator(`li[data-asset="${before}"]`).getByRole("checkbox")).toBeChecked();
  // Samme med tast 3 (bildeområdet har fokus).
  const next = await items(page).nth(2).getAttribute("data-asset");
  await page.locator(`[data-name="${pick}"]`).press("3");
  await expect(items(page).first()).toHaveAttribute("data-asset", next!);
});

test("synkronisert zoom: knapper, taster og Ctrl + hjul gir samme zoom på alle bildene", async ({ page }) => {
  await scanned(page, "#/series/41");
  const zooms = () => page.locator(".img").evaluateAll((els) => els.map((e) => (e as HTMLElement).style.getPropertyValue("--z")));
  expect(await zooms()).toEqual(["1", "1", "1", "1"]);
  await page.getByRole("button", { name: "Zoom inn" }).click();
  expect(await zooms()).toEqual(["1.5", "1.5", "1.5", "1.5"]);
  await expect(page.locator(".zlevel")).toHaveText("Zoom 150 %");
  await page.locator('[data-name]').first().focus();
  await page.keyboard.press("+");
  expect(await zooms()).toEqual(["2", "2", "2", "2"]);
  await page.keyboard.press("-");
  await page.keyboard.press("-");
  expect(await zooms()).toEqual(["1", "1", "1", "1"]);
  await page.locator("ul.items").hover();
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -100);
  await page.keyboard.up("Control");
  await expect.poll(zooms).toEqual(["1.5", "1.5", "1.5", "1.5"]);
  await page.getByRole("button", { name: "Tilbakestill zoom" }).click();
  expect(await zooms()).toEqual(["1", "1", "1", "1"]);
  // Zoomede bilder kan ruller med tastatur og følger hverandre.
  await page.getByRole("button", { name: "Zoom inn" }).click();
  await page.getByRole("button", { name: "Zoom inn" }).click();
  await items(page).first().locator(".img").evaluate((el) => { el.scrollLeft = 40; });
  await expect.poll(() => items(page).nth(2).locator(".img").evaluate((el) => el.scrollLeft)).toBeGreaterThan(0);
});

test("lukkede øyne er et tydelig tekstmerke, og øyne-forklaringen vises bare når øyne ikke er vurdert", async ({ page }) => {
  await scanned(page, "#/series/42");
  await expect(page.locator("ul.items .meta").getByText("Lukkede øyne", { exact: true })).toHaveCount(1);
  await expect(page.getByTestId("eyes-unavailable")).toHaveCount(0);
  await expect(items(page).first().locator('[data-info="reason"]')).toHaveText("Åpne øyne");
  await go(page, "#/series/41");
  await expect(page.getByTestId("eyes-unavailable")).toContainText("Øyne er ikke vurdert");
});

test("beskyttelse: bryter per bilde med angre; beskyttede er aldri forhåndsmerket", async ({ page }) => {
  await scanned(page, "#/series/41");
  const second = items(page).nth(1);
  await second.getByRole("button", { name: "Beskytt bilde 2" }).click();
  await expect(second.getByRole("button", { name: "Beskytt bilde 2" })).toHaveAttribute("aria-pressed", "true");
  await expect(second.getByText("Beskyttet. Beholdes alltid.")).toBeVisible();
  await expect(second.getByRole("checkbox")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Flytt 2 til papirkurv" })).toBeVisible();
  await page.getByRole("status").filter({ hasText: "Bilde 2 er beskyttet." }).getByRole("button", { name: "Angre" }).click();
  await expect(items(page).nth(1).getByRole("button", { name: "Beskytt bilde 2" })).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByRole("button", { name: "Flytt 3 til papirkurv" })).toBeVisible();
});

test("Kortmodus og Gjennomgang forklarer serie- og regelforslag med tall", async ({ page }) => {
  await scanned(page, "#/cards/series");
  await expect(page.getByText("Hvorfor foreslås dette?")).toBeVisible();
  await expect(page.locator(".explain").getByText("Plass i serien")).toBeVisible();
  await expect(page.locator(".explain").getByText("Appen foreslår. Du bestemmer.")).toBeVisible();
  await go(page, "#/");
  await go(page, "#/cards/rule");
  await expect(page.locator(".explain").getByText(/Gamle skjermbilder/).first()).toBeVisible();
  // Serier og regler kan gjennomgås i listen og flyttes som ett angrebart valg (åpent spørsmål 16).
  await go(page, "#/review/series");
  await page.getByRole("button", { name: /serie|bilder/i }).first().waitFor();
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt \d+/ }).click();
  await expect(page.getByText(/flyttet/)).toBeVisible();
  await go(page, "#/review/rule");
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt \d+/ }).click();
  await expect(page.getByText(/flyttet/)).toBeVisible();
});

test("Gruppe viser forklaring på forslagene", async ({ page }) => {
  await scanned(page, "#/group/1");
  await expect(page.locator(".explain").first()).toContainText("Hvorfor foreslås dette?");
});

test("pseudo og smal skjerm: ingen avkutting og ingen sideveis rulling", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 740 });
  await scanned(page, "#/series/42", "pseudo");
  const problems = await page.evaluate(() => {
    const out: string[] = [];
    const de = document.documentElement;
    if (de.scrollWidth > de.clientWidth + 1) out.push(`horisontal rulling ${de.scrollWidth} > ${de.clientWidth}`);
    for (const el of Array.from(document.querySelectorAll(".items *, .zoombar *, .lead"))) {
      if (el.closest(".sr-only") || el instanceof SVGElement) continue;
      const cs = getComputedStyle(el);
      if (cs.display === "none" || cs.textOverflow === "ellipsis" || el.matches(".img, .img *")) continue;
      if (el.scrollWidth > el.clientWidth + 1 && cs.overflowX !== "visible") out.push(`avkuttet: ${el.className} «${(el.textContent ?? "").slice(0, 25)}»`);
      if (el.matches("button, label, dd, dt, p") && el.scrollWidth > el.clientWidth + 1) out.push(`renner over: ${el.tagName} «${(el.textContent ?? "").slice(0, 25)}»`);
    }
    return out.slice(0, 8);
  });
  expect(problems).toEqual([]);
  await page.screenshot({ path: "e2e/.results/series-pseudo-smal.png", fullPage: true });
});

test("engelsk: flyten og navn på knappene", async ({ page }) => {
  await scanned(page, "#/series/41", "en");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Compare the photos in the series");
  await expect(page.getByRole("button", { name: "Move 3 to trash" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Keep all" })).toBeVisible();
  await expect(items(page).first().getByText("Suggested to keep")).toBeVisible();
  await expect(page.getByRole("button", { name: "Zoom in" })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "Move to trash" })).toHaveCount(3);
});

const VIEWPORTS = { bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } } as const;
for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`axe ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await scanned(page, "#/series/41");
      await axe(page, "serie 41");
      await page.getByRole("button", { name: "Zoom inn" }).click();
      await axe(page, "serie 41 zoomet");
      await items(page).nth(1).getByRole("button", { name: "Beskytt bilde 2" }).click();
      await expect(items(page).nth(1).getByText("Beskyttet. Beholdes alltid.")).toBeVisible();
      await axe(page, "serie 41 med beskyttet bilde");
      await go(page, "#/series/42");
      await axe(page, "serie 42 lukkede øyne");
      await page.getByRole("button", { name: /^Flytt \d+ til papirkurv/ }).click();
      await expect(page.getByText(/Serien er ferdig/)).toBeVisible();
      await axe(page, "serie ferdig");
      for (const hash of ["#/cards/series", "#/cards/rule", "#/review/series", "#/review/rule", "#/group/1"]) {
        await go(page, hash);
        await page.waitForTimeout(100);
        await axe(page, hash);
      }
    });
  }
}

test("skjermbilder (SHOTS=1)", async ({ page }) => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  test.setTimeout(180_000);
  for (const lang of ["nb", "en"] as const) {
    for (const scheme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await page.setViewportSize({ width: 1280, height: 900 });
      await scanned(page, "#/series/42", lang);
      await page.waitForTimeout(400);
      await page.screenshot({ path: `../docs/screens/series/series-${lang}-${scheme}.png`, fullPage: true });
    }
  }
});
