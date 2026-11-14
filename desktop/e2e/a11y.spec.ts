// axe-core mot hver skjerm og viktige tilstander: lys/mørk × desktop/smal, 0 brudd (WCAG 2.2 AA).
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open, seedScan } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

async function axe(page: Page, where: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const res = await page.evaluate(async (tags) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; help: string; nodes: { target: unknown[]; failureSummary?: string }[] }) =>
      ({ id: v.id, help: v.help, nodes: v.nodes.slice(0, 3).map((n) => `${n.target.join(" ")} :: ${n.failureSummary?.split("\n")[1] ?? ""}`) }));
  }, TAGS);
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

const VIEWPORTS = { desktop: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } } as const;

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`axe ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await open(page, "#/onboarding");
      await axe(page, "onboarding velkommen");
      await page.getByRole("button", { name: "Kom i gang" }).click();
      await axe(page, "onboarding mapper");
      await page.getByRole("button", { name: "Start skanning" }).click();
      await page.getByRole("button", { name: /^Flytt \d+/ }).waitFor();
      await axe(page, "onboarding resultat");

      await go(page, "#/");
      await page.getByRole("button", { name: "Rask opprydding" }).waitFor();
      await axe(page, "hjem");
      for (const [hash, name] of [
        ["#/review", "gjennomgang kategorier"], ["#/review/blurry", "gjennomgang uskarpe"],
        ["#/review/screenshot", "gjennomgang skjermbilder"], ["#/review/exact_dup", "gjennomgang eksakte"],
        ["#/review/visual_dup", "gjennomgang like"], ["#/group/1", "gruppe"], ["#/cards", "kortmodus"], ["#/settings", "innstillinger"],
      ] as const) {
        await go(page, hash);
        await page.waitForTimeout(100);
        await axe(page, name);
      }
      // Dialoger
      await go(page, "#/cards");
      await page.getByRole("button", { name: "Hurtigtaster" }).click();
      await expect(page.getByRole("dialog")).toBeVisible();
      await axe(page, "kortmodus F1-dialog");
      await page.keyboard.press("Escape");
      await go(page, "#/review/blurry");
      await page.getByRole("button", { name: "Velg alle" }).click();
      await page.getByRole("button", { name: /^Flytt 6/ }).click();
      await expect(page.getByText("6 flyttet")).toBeVisible();
      await axe(page, "gjennomgang etter flytt");
      await go(page, "#/trash");
      await expect(page.locator(".summary")).toBeVisible();
      await axe(page, "papirkurv");
      await page.getByRole("button", { name: "Tøm papirkurv …" }).click();
      await axe(page, "tøm-ark");
      await page.getByRole("radio", { name: "Slett permanent" }).check();
      await page.getByRole("button", { name: "Fortsett" }).click();
      await axe(page, "permanent-trinn");
    });
  }
}

test("axe: kortmodus-slutt, tom papirkurv og Hjem med feilfri tomtilstand", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await open(page, "#/");
  await axe(page, "hjem før skann");
  await seedScan(page);
  await go(page, "#/cards");
  await page.keyboard.press("Tab");
  for (let i = 0; i < 40 && (await page.getByRole("heading", { name: "Ingen flere kort" }).count()) === 0; i++) {
    await page.keyboard.press("ArrowLeft");
    await page.waitForTimeout(10);
  }
  await axe(page, "kortmodus slutt");
  await go(page, "#/trash");
  await axe(page, "papirkurv tom");
});
