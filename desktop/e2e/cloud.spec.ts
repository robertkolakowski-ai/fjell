// Skymappe-advarsel (M9): banner på Hjem, Gjennomgang, Gruppe og Kortmodus; papirkurv sperret synlig med forklaring
// (aria-disabled + aria-describedby, ikke skjult); Behold virker. MockBridge styres med `?mockLibrary=...` (bare dev).
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open, type Lang } from "./helpers";

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];
const CLOUD = "&mockLibrary=" + encodeURIComponent("C:\\Users\\a\\OneDrive - Firma AS\\Bilder");

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

async function start(page: Page, lang: Lang = "nb", query = CLOUD): Promise<void> {
  await open(page, "#/", lang, 0, query);
  // Uavhengig av språk (pseudo har andre ord): knappene i onboarding finnes på samme sted.
  await go(page, "#/onboarding");
  await page.locator(".onb > .row .btn.primary").click();
  await page.locator(".onb > .row .btn.primary").click();
  await page.locator(".onb .btn.primary").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 }); // resultatsteget: skannet er ferdig
  await go(page, "#/");
}

/** Knappen er sperret synlig: aria-disabled, fokuserbar, og beskrivelsen peker på forklaringen i banneret. */
async function expectBlocked(page: Page, name: RegExp): Promise<void> {
  const btn = page.getByRole("button", { name });
  await expect(btn).toHaveAttribute("aria-disabled", "true");
  // Playwright regner aria-disabled som deaktivert: sjekk i stedet at det ikke finnes noe `disabled`-attributt (knappen kan fokuseres).
  expect(await btn.evaluate((e) => (e as HTMLButtonElement).disabled)).toBe(false);
  await expect(btn).toHaveAccessibleDescription(/\S/);
  const why = await page.locator("#cloud-why").textContent();
  expect(await btn.evaluate((e) => e.getAttribute("aria-describedby"))).toBe("cloud-why");
  expect(why).toMatch(/slått av|turned off/);
  await btn.focus();
  await expect(btn).toBeFocused();
}

test("banner på Hjem, Gjennomgang, Gruppe og Kortmodus med forklaring og tips (nb)", async ({ page }) => {
  await start(page);
  for (const hash of ["#/", "#/review", "#/review/blurry", "#/group/1", "#/cards"]) {
    await go(page, hash);
    const banner = page.locator("#cloud-warning");
    await expect(banner, hash).toBeVisible();
    await expect(banner.getByRole("heading", { level: 2 })).toHaveText("Mappen synkroniseres med OneDrive");
    await expect(banner).toContainText("slått av");
    await expect(banner).toContainText("lokal kopi");
    await expect(banner).toContainText("kan vi ikke se det"); // ærlig forbehold: navnebasert
  }
});

test("banner på engelsk, og iCloud/Dropbox gir riktig navn", async ({ page }) => {
  await start(page, "en", "&mockLibrary=" + encodeURIComponent("/Users/a/iCloud Drive/Photos"));
  await expect(page.locator("#cloud-warning h2")).toHaveText("This folder syncs with iCloud");
  await page.goto("/?mockDelay=0&mockLibrary=" + encodeURIComponent("D:\\Dropbox (Personal)\\Bilder") + "#/");
  await expect(page.locator("#cloud-warning h2")).toHaveText("This folder syncs with Dropbox");
});

test("lokalt bibliotek: ingen advarsel, papirkurv virker som før", async ({ page }) => {
  await start(page, "nb", "");
  await go(page, "#/review/blurry");
  await page.locator(".tile").first().waitFor();
  await expect(page.locator("#cloud-warning")).toHaveCount(0);
  await page.getByRole("button", { name: "Velg alle" }).click();
  const move = page.getByRole("button", { name: /^Flytt 6/ });
  await expect(move).not.toHaveAttribute("aria-disabled", "true");
  await move.click();
  await expect(page.getByText("6 flyttet")).toBeVisible();
});

test("Gjennomgang: knapp og Delete gjør ingenting, men forklarer. Ingen skjult sperre", async ({ page }) => {
  await start(page);
  await go(page, "#/review/blurry");
  await page.locator(".tile").first().waitFor();
  await page.getByRole("button", { name: "Velg alle" }).click();
  await expectBlocked(page, /^Flytt 6/);
  // Mus: klikk gjør ingenting.
  await page.getByRole("button", { name: /^Flytt 6/ }).click({ force: true }); // force: Playwright vil ikke klikke aria-disabled
  await expect(page.getByText("6 flyttet")).toHaveCount(0);
  // Tastatur: Delete med fokus i listen gjør ingenting, men forklarer (synlig og i live-region).
  await page.locator(".tile").first().focus();
  await page.keyboard.press("Delete");
  await expect(page.locator(".note")).toContainText("Papirkurv-tasten gjør ingenting");
  await expect(page.locator("[role=status]", { hasText: "Papirkurv-tasten gjør ingenting" })).toHaveCount(1);
  await expect(page.getByText("6 flyttet")).toHaveCount(0);
  // Ingenting er flyttet: Papirkurv er tom.
  await go(page, "#/trash");
  await expect(page.locator(".trash")).not.toContainText("IMG_");
  await go(page, "#/review/blurry");
  await expect(page.locator(".tile")).toHaveCount(6);
});

test("Gruppe: «Behold dette, flytt N» er sperret med forklaring, «Behold alle» virker", async ({ page }) => {
  await start(page);
  await go(page, "#/group/1");
  await page.locator(".items").waitFor();
  await expectBlocked(page, /^Behold dette, flytt/);
  await page.getByRole("button", { name: /^Behold dette, flytt/ }).click({ force: true });
  await expect(page.getByText(/flyttet/)).toHaveCount(0);
  await page.getByRole("button", { name: "Behold alle" }).click();
  await expect(page).toHaveURL(/#\/review$/);
});

test("Kortmodus: papirkurv-taster (Delete, pil opp) og knapp gjør ingenting men forklarer, Behold virker", async ({ page }) => {
  await start(page);
  await go(page, "#/cards");
  const deck = page.locator(".deck");
  await deck.waitFor();
  const remaining = () => page.locator("p.muted[aria-hidden=true]").first().textContent();
  const before = await remaining();
  await expectBlocked(page, /^Papirkurv$/);
  await deck.focus();
  for (const key of ["Delete", "ArrowUp"]) {
    await page.keyboard.press(key);
    await expect(page.locator(".note")).toContainText("Papirkurv-tasten gjør ingenting");
    expect(await remaining(), key).toBe(before);
  }
  await page.getByRole("button", { name: /^Papirkurv$/ }).click({ force: true });
  expect(await remaining()).toBe(before);
  // Behold virker (pil venstre), forklaringen forsvinner.
  await deck.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page.locator(".note")).toHaveCount(0);
  await expect.poll(remaining).not.toBe(before);
  await page.getByRole("button", { name: /^Behold$/ }).click();
  await expect.poll(remaining).not.toBe(before);
  // Hopp over virker også.
  const mid = await remaining();
  await page.getByRole("button", { name: /^Hopp over$/ }).click();
  await expect.poll(remaining).toBe(mid); // hopp over flytter kortet bakerst: antall igjen er uendret
});

test("ny mappe i skymappe via «Legg til mappe»: banner dukker opp på Hjem og papirkurv sperres", async ({ page }) => {
  await start(page, "nb", "");
  await go(page, "#/");
  await expect(page.locator("#cloud-warning")).toHaveCount(0);
  await page.getByRole("button", { name: "Legg til mappe" }).click();
  await page.getByLabel("Mappens sti").fill("C:\\Users\\a\\Dropbox\\Bilder");
  await page.getByRole("button", { name: "Legg til", exact: true }).click();
  await expect(page.locator("#cloud-warning h2")).toHaveText("Mappen synkroniseres med Dropbox");
  await go(page, "#/cards");
  await page.locator(".deck").waitFor();
  await expectBlocked(page, /^Papirkurv$/);
});

// Pseudo-lokale (nb +30 %): ingen avkutting og ingen sidelengs rulling i banneret og i de sperrede knappene.
test("pseudo-lokale: banner og sperrede knapper kuttes ikke (bred og smal)", async ({ page }) => {
  for (const [w, h] of [[1000, 700], [360, 740]] as const) {
    await page.setViewportSize({ width: w, height: h });
    await start(page, "pseudo");
    for (const hash of ["#/", "#/review/blurry", "#/group/1", "#/cards"]) {
      await go(page, hash);
      await page.locator("#cloud-warning").waitFor();
      await page.waitForTimeout(100);
      const bad = await page.evaluate(() => {
        const out: string[] = [];
        const root = document.documentElement;
        if (root.scrollWidth > root.clientWidth + 1) out.push(`sidelengs rulling ${root.scrollWidth}>${root.clientWidth}`);
        for (const el of Array.from(document.querySelectorAll<HTMLElement>("#cloud-warning, #cloud-warning *, button[aria-disabled=true], .note"))) {
          if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).display !== "inline") out.push(`${el.tagName}.${el.className} bredde`);
          if (el.scrollHeight > el.clientHeight + 1 && getComputedStyle(el).display !== "inline") out.push(`${el.tagName}.${el.className} høyde`);
          if (/hidden|clip/.test(getComputedStyle(el).overflow) && el.id !== "cloud-warning" && el.tagName !== "BUTTON") out.push(`${el.tagName} skjuler overflow`);
        }
        const t = document.getElementById("cloud-warning")?.textContent ?? "";
        if (!t.includes("[") || !t.includes("]")) out.push("ikke pseudo-tekst");
        return out;
      });
      expect(bad, `${w}px ${hash}`).toEqual([]);
    }
  }
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of [["bred", { width: 1280, height: 800 }], ["smal", { width: 360, height: 740 }]] as const) {
    test(`axe skymappe ${scheme} ${vp}: 0 brudd`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await start(page);
      for (const [hash, name] of [["#/", "hjem"], ["#/review", "gjennomgang"], ["#/review/blurry", "uskarpe"],
        ["#/review/exact_dup", "eksakte"], ["#/group/1", "gruppe"], ["#/cards", "kortmodus"]] as const) {
        await go(page, hash);
        await page.locator("#cloud-warning").waitFor();
        await axe(page, `skymappe ${name}`);
      }
      // Etter Delete-forklaring og med valg.
      await go(page, "#/review/blurry");
      await page.getByRole("button", { name: "Velg alle" }).click();
      await page.locator(".tile").first().focus();
      await page.keyboard.press("Delete");
      await page.locator(".note").waitFor();
      await axe(page, "uskarpe med forklaring");
    });
  }
}

// Skjermbilder til docs/screens/m9/ (bare med SHOTS=1). JPEG, liten skala for å holde filene små.
test.describe("skjermbilder skymappe", () => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  const OUT = process.env.SHOTS_DIR ?? "../docs/screens/m9";
  const shot = async (page: Page, name: string): Promise<void> => {
    await page.waitForTimeout(250);
    await page.screenshot({ path: `${OUT}/${name}.jpg`, type: "jpeg", quality: 55 });
  };
  const tour = async (page: Page, lang: Lang, scheme: "light" | "dark", size: { width: number; height: number }): Promise<void> => {
    await page.setViewportSize(size);
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
    await start(page, lang);
    const tag = `${lang}-${scheme}-${size.width < 600 ? "smal" : "bred"}`;
    await go(page, "#/");
    await shot(page, `skymappe-hjem-${tag}`);
    await go(page, "#/review/blurry");
    await page.locator(".tile").first().waitFor();
    await page.locator(".tile").first().click();
    await page.locator(".tile").first().focus();
    await page.keyboard.press("Delete");
    await page.locator(".note").waitFor();
    await shot(page, `skymappe-gjennomgang-${tag}`);
    await go(page, "#/group/1");
    await page.locator(".items").waitFor();
    await shot(page, `skymappe-gruppe-${tag}`);
    await go(page, "#/cards");
    await page.locator(".deck").waitFor();
    await page.locator(".deck").focus();
    await page.keyboard.press("Delete");
    await page.locator(".note").waitFor();
    await shot(page, `skymappe-kortmodus-${tag}`);
  };
  test("nb lys bred", async ({ page }) => { test.setTimeout(120_000); await tour(page, "nb", "light", { width: 900, height: 700 }); });
  test("nb mørk bred", async ({ page }) => { test.setTimeout(120_000); await tour(page, "nb", "dark", { width: 900, height: 700 }); });
  test("en lys smal", async ({ page }) => { test.setTimeout(120_000); await tour(page, "en", "light", { width: 390, height: 800 }); });
  test("pseudo lys smal", async ({ page }) => { test.setTimeout(120_000); await tour(page, "pseudo", "light", { width: 390, height: 800 }); });
});
