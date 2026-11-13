// V2-skall og felles komponenter: lokal skrift, merke, sidemeny, bevegelse, tomme tilstander/skjelett i galleriet (axe, høykontrast, pseudo).
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { open } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

async function axe(page: Page, where: string, ctx = "html"): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  const res = await page.evaluate(async ({ tags, ctx }) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document.querySelector(ctx) ?? document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; nodes: { target: unknown[] }[] }) => ({ id: v.id, nodes: v.nodes.slice(0, 3).map((n) => n.target.join(" ")) }));
  }, { tags: TAGS, ctx });
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

test("skrift: Inter Variable lastes fra egen opprinnelse (ingen eksterne forespørsler)", async ({ page }) => {
  const external: string[] = [];
  const fonts: string[] = [];
  page.on("request", (r) => {
    const u = new URL(r.url());
    if (u.hostname !== "localhost") external.push(r.url());
    if (/\.woff2?$/.test(u.pathname)) fonts.push(u.pathname);
  });
  await open(page, "#/", "nb");
  await page.evaluate(() => document.fonts.load('600 16px "Inter Variable"', "Fjellæøå"));
  expect(await page.evaluate(() => document.fonts.check('600 16px "Inter Variable"', "Fjellæøå"))).toBe(true);
  expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toMatch(/^"?Inter Variable/);
  expect(fonts.length).toBeGreaterThan(0);
  expect(external).toEqual([]);
});

test("sidemeny: symbol og ordmerke uten boks; menykolonnen når hele dokumenthøyden", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await open(page, "#/settings", "nb");
  const brand = page.locator("header .brand");
  await expect(brand.locator("svg.logo")).toBeVisible();
  await expect(brand.locator(".name")).toHaveText("Fjell");
  expect(await brand.locator("span.logo").evaluate((e) => getComputedStyle(e).backgroundColor)).toBe("rgba(0, 0, 0, 0)");
  expect(await page.locator(".app").evaluate((e) => getComputedStyle(e).backgroundImage)).toContain("linear-gradient");
  // Aktiv navigasjon: fylt ikon + aria-current
  await expect(page.locator("nav a[aria-current=page] svg.icon.filled")).toHaveCount(1);
  await page.setViewportSize({ width: 360, height: 740 });
  expect(await page.locator(".app").evaluate((e) => getComputedStyle(e).backgroundImage)).not.toContain("gradient(to right");
});

test("bevegelse: sidebytte toner inn, men ikke ved redusert bevegelse", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await open(page, "#/settings", "nb");
  expect(await page.locator("main > *").first().evaluate((e) => getComputedStyle(e).animationName)).toBe("view-in");
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--dur-1").trim())).toBe("90ms");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.evaluate(() => { location.hash = "#/trash"; });
  await page.locator("h1").first().waitFor();
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--dur-1").trim())).toBe("0ms");
  const anim = await page.locator("main > *").first().evaluate((e) => ({ d: getComputedStyle(e).animationDuration, n: getComputedStyle(e).animationName }));
  expect(anim.n === "none" || parseFloat(anim.d) < 0.001).toBe(true);
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of [["bred", { width: 1280, height: 800 }], ["smal", { width: 360, height: 740 }]] as const) {
    test(`galleri V2 (tomme tilstander, skjelett, scene, bannere): axe ${scheme} ${vp}, ingen horisontal rulling`, async ({ page }) => {
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await page.goto("/?gallery");
      await page.locator(".v2 .empties").waitFor();
      await axe(page, `galleri ${scheme} ${vp}`, ".v2"); // galleriets eldre flislinje har egne funn (label-content-name-mismatch), utenfor V2-omfanget
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
    });
  }
}

test("galleri V2: pseudo-lokale og høykontrast (forced-colors) beholder kontur og tekst", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await page.setViewportSize({ width: 360, height: 740 });
  await page.addInitScript(() => { try { localStorage.setItem("fjell.language", "pseudo"); } catch { /* ignorer */ } });
  await page.goto("/?gallery");
  await page.locator(".v2 .empties").waitFor();
  const b = await page.locator(".v2 .skels .block").evaluate((e) => getComputedStyle(e).borderTopWidth);
  expect(parseFloat(b)).toBeGreaterThan(0);
  expect(await page.locator(".v2 .contours path.acc").first().evaluate((e) => getComputedStyle(e).stroke)).not.toBe("none");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  // Scenens knapper: kant synlig
  expect(parseFloat(await page.locator(".v2 .stage .btn").first().evaluate((e) => getComputedStyle(e).borderTopWidth))).toBeGreaterThan(0);
});
