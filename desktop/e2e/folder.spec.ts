// «Bla …» i «Legg til mappe» (Hjem og Onboarding): fyller feltet via MockBridge (`?mockPick=`), avbrutt endrer ikke feltet,
// axe 0 brudd (lys/mørk × bred/smal), pseudo-lokale uten avkutting, tastaturflyt. Skjermbilde med SHOTS=1.
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import type { Lang } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];
const PICKED = "C:\\Users\\Ola\\OneDrive\\Bilder\\Ferie";

/** `pick`: sti = valgt mappe, "cancel" = avbrutt, undefined = mappevalg utilgjengelig (nettleser). */
async function openAt(page: Page, hash: string, pick?: string, lang: Lang = "nb"): Promise<void> {
  await page.addInitScript((l) => { try { localStorage.setItem("fjell.language", l); } catch { /* ignorer */ } }, lang);
  const q = pick === undefined ? "" : `&mockPick=${encodeURIComponent(pick)}`;
  await page.goto(`/?mockDelay=0${q}${hash}`);
  await page.locator("h1").first().waitFor();
}

async function openHomeDialog(page: Page, name: RegExp = /^(Legg til mappe|Add folder)$/): Promise<void> {
  await page.getByRole("button", { name }).first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
}

async function axe(page: Page, where: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const res = await page.evaluate(async (tags) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document, { runOnly: { type: "tag", values: tags } });
    return r.violations.map((v: { id: string; nodes: { target: unknown[] }[] }) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(" | ")}`);
  }, TAGS);
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

test("Bla fyller feltet, annonserer valgt sti og flytter fokus til feltet; biblioteket endres først ved «Legg til»", async ({ page }) => {
  await openAt(page, "#/", PICKED);
  await openHomeDialog(page);
  const field = page.getByRole("textbox", { name: "Mappens sti" });
  await expect(field).toHaveValue("");
  await page.getByRole("button", { name: "Bla …" }).click();
  await expect(field).toHaveValue(PICKED);
  await expect(field).toBeFocused();
  await expect(page.getByRole("dialog").getByRole("status")).toContainText(`Valgt: ${PICKED}`);
  // ikke bekreftet ennå: Hjem viser fortsatt standardmappen
  await expect(page.getByText("C:\\Bilder", { exact: false }).first()).toBeAttached();
  await page.getByRole("button", { name: "Legg til", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(page.getByText(`Mappe: ${PICKED}`)).toBeVisible();
});

test("avbrutt valg endrer ikke feltet (heller ikke det brukeren skrev), og gir ingen feil", async ({ page }) => {
  await openAt(page, "#/", "cancel");
  await openHomeDialog(page);
  const field = page.getByRole("textbox", { name: "Mappens sti" });
  await field.fill("D:\\Skrevet");
  await page.getByRole("button", { name: "Bla …" }).click();
  await expect(field).toHaveValue("D:\\Skrevet");
  await expect(field).toBeFocused();
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("uten mappevalg (nettleser): Bla er deaktivert med forklaring, manuell inntasting virker", async ({ page }) => {
  await openAt(page, "#/");
  await openHomeDialog(page);
  await expect(page.getByRole("button", { name: "Bla …" })).toBeDisabled();
  await expect(page.getByRole("dialog").getByText("Mappevalg er ikke tilgjengelig her. Skriv inn stien.")).toBeVisible();
  await page.getByRole("textbox", { name: "Mappens sti" }).fill("D:\\Manuelt");
  await page.getByRole("button", { name: "Legg til", exact: true }).click();
  await expect(page.getByText("D:\\Manuelt")).toBeVisible();
});

test("tastaturflyt: Tab til Bla, Enter, fokus i feltet, Enter bekrefter; Esc lukker uten endring", async ({ page }) => {
  await openAt(page, "#/", PICKED);
  await openHomeDialog(page);
  const field = page.getByRole("textbox", { name: "Mappens sti" });
  await field.focus();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Bla …" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(field).toHaveValue(PICKED);
  await expect(field).toBeFocused();
  await page.keyboard.press("Enter"); // sender skjemaet = «Legg til»
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(page.getByText(`Mappe: ${PICKED}`)).toBeVisible();
  // Esc: ingen endring
  await openHomeDialog(page);
  await page.getByRole("button", { name: "Bla …" }).press("Space");
  await expect(field).toHaveValue(PICKED);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
});

test("onboarding bruker samme dialog: Bla fyller feltet og Legg til setter mappen", async ({ page }) => {
  await openAt(page, "#/onboarding", PICKED);
  await page.getByRole("button", { name: "Kom i gang" }).click();
  await page.getByRole("button", { name: "Legg til mappe" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "Bla …" }).click();
  await expect(page.getByRole("textbox", { name: "Mappens sti" })).toHaveValue(PICKED);
  await page.getByRole("button", { name: "Legg til", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
});

test("engelsk: Browse …", async ({ page }) => {
  await openAt(page, "#/", PICKED, "en");
  await openHomeDialog(page);
  await page.getByRole("button", { name: "Browse …" }).click();
  await expect(page.getByRole("textbox", { name: "Folder path" })).toHaveValue(PICKED);
  await expect(page.getByRole("dialog").getByRole("status")).toContainText("Chosen:");
});

for (const lang of ["nb", "pseudo"] as const) {
  for (const [vp, size] of Object.entries({ bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } })) {
    test(`layout uten avkutting: ${lang} ${vp}, med og uten mappevalg`, async ({ page }) => {
      await page.setViewportSize(size);
      for (const pick of [PICKED, undefined]) {
        await openAt(page, "#/", pick, lang);
        await openHomeDialog(page, /./);
        if (pick) await page.getByRole("dialog").getByRole("button").first().click();
        await page.waitForTimeout(100);
        const problems = await page.evaluate(() => {
          const out: string[] = [];
          const d = document.querySelector("dialog")!;
          const dr = d.getBoundingClientRect();
          if (dr.left < -1 || dr.right > innerWidth + 1) out.push(`dialog utenfor vinduet ${dr.left}..${dr.right}`);
          if (d.scrollWidth > d.clientWidth + 1) out.push("dialog: horisontal rulling");
          for (const el of Array.from(d.querySelectorAll("*"))) {
            if (el.closest(".sr-only")) continue;
            const r = el.getBoundingClientRect();
            if (r.width === 0) continue;
            if (r.right > dr.right + 1 || r.left < dr.left - 1) out.push(`${el.tagName} utenfor dialogen`);
            if (el.matches("button, input") && el.scrollWidth > el.clientWidth + 1 && el.tagName === "BUTTON") out.push(`knapp renner over: ${el.textContent}`);
          }
          return out;
        });
        expect(problems).toEqual([]);
      }
    });
  }
}

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries({ bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } })) {
    test(`axe: Legg til mappe ${scheme} ${vp}`, async ({ page }) => {
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await openAt(page, "#/", PICKED);
      await openHomeDialog(page);
      await axe(page, "dialog før valg");
      await page.getByRole("button", { name: "Bla …" }).click();
      await axe(page, "dialog etter valg");
      await page.keyboard.press("Escape");
      await openAt(page, "#/"); // uten mappevalg
      await openHomeDialog(page);
      await axe(page, "dialog uten mappevalg");
    });
  }
}

test("skjermbilde: dialogen med valgt mappe", async ({ page }) => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  await page.setViewportSize({ width: 900, height: 640 });
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await openAt(page, "#/", "D:\\Bilder\\Ferie 2025");
  await openHomeDialog(page);
  await page.getByRole("button", { name: "Bla …" }).click();
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${process.env.SHOTS_DIR ?? "../docs/screens/m9"}/legg-til-mappe-bla-lys.jpg`, type: "jpeg", quality: 60 });
});
