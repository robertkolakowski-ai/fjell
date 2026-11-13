// Oppdateringer (M9): Innstillinger «Oppdateringer», banner på Hjem, alle mockUpdate-tilstander, skanning blokkerer installasjon,
// tastaturflyt og axe (0 brudd, lys/mørk × bred/smal). MockBridge i nettleser; selve oppdateringen (Tauri) testes på Windows.
// Skjermbilder (en per hovedtilstand) lagres bare med SHOTS=1: `SHOTS=1 npx playwright test e2e/update.spec.ts` til docs/screens/m9/.
import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import { go, open } from "./helpers";

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
  }, tags());
  expect(res, `axe-brudd: ${where}`).toEqual([]);
  function tags() { return TAGS; }
}

const check = (page: Page) => page.getByRole("button", { name: "Se etter oppdatering" });
const install = (page: Page) => page.getByRole("button", { name: "Installer og start på nytt" });
const section = (page: Page) => page.getByRole("region", { name: "Oppdateringer" });
const status = (page: Page) => section(page).locator(".status");

test("standard: versjon vises, ingen sjekk er gjort, auto er av, ingen installasjonsknapp", async ({ page }) => {
  await open(page, "#/settings", "nb", 0, "&mockUpdate=available");
  await expect(section(page).getByText("Versjon dev")).toBeVisible();
  await expect(check(page)).toBeEnabled();
  await expect(install(page)).toHaveCount(0);
  await expect(status(page)).toHaveCount(0);
  const auto = page.getByRole("switch", { name: "Sjekk automatisk ved oppstart" });
  await expect(auto).not.toBeChecked();
  // Forklaringen sier hva som sendes, og er knyttet til bryteren.
  await expect(auto).toHaveAccessibleDescription(/kun appversjon og operativsystem, aldri bilder, stier eller filnavn/);
  // Ingen sjekk uten klikk: Hjem viser ikke banner.
  await go(page, "#/");
  await expect(page.getByText(/Ny versjon .* er tilgjengelig/)).toHaveCount(0);
});

test("ingen ny versjon: melding i live region", async ({ page }) => {
  await open(page, "#/settings", "nb", 0, "&mockUpdate=none");
  await check(page).click();
  await expect(status(page)).toHaveText("Du har nyeste versjon.");
  await expect(section(page).locator("[role=status][aria-live=polite]")).toHaveText("Du har nyeste versjon.");
  await expect(install(page)).toHaveCount(0);
});

test("ny versjon: versjon, notater og dato vises, installasjon krever eget klikk, så fremdrift og omstart", async ({ page }) => {
  await open(page, "#/settings", "nb", 0, "&mockUpdate=available");
  await check(page).click();
  await expect(status(page)).toHaveText("Ny versjon 0.1.99 er tilgjengelig.");
  await expect(section(page).getByText("Rettet en feil i Papirkurv.")).toBeVisible();
  await expect(section(page).getByText("Utgitt 12. november 2026")).toBeVisible();
  await expect(install(page)).not.toHaveAttribute("aria-disabled", "true");
  // Ingenting skjer av seg selv.
  await page.waitForTimeout(500);
  await expect(section(page).getByRole("progressbar")).toHaveCount(0);
  await install(page).click();
  await expect(status(page)).toHaveText("Oppdateringen er installert. Appen starter på nytt …");
  const bar = section(page).getByRole("progressbar", { name: "Fremdrift for oppdateringen" });
  await expect(bar).toHaveAttribute("aria-valuenow", "100");
});

test("slow: progressbar har riktig ARIA og stiger", async ({ page }) => {
  await open(page, "#/settings", "nb", 0, "&mockUpdate=slow");
  await check(page).click();
  await install(page).click();
  const bar = section(page).getByRole("progressbar", { name: "Fremdrift for oppdateringen" });
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute("aria-valuemin", "0");
  await expect(bar).toHaveAttribute("aria-valuemax", "100");
  await expect(status(page)).toHaveText("Laster ned og installerer versjon 0.1.99 …");
  const seen = new Set<number>();
  for (let i = 0; i < 30 && seen.size < 3; i++) {
    const v = await bar.getAttribute("aria-valuenow");
    if (v !== null) seen.add(Number(v));
    await page.waitForTimeout(150);
  }
  expect(seen.size, `fremdrift sett: ${[...seen].join(",")}`).toBeGreaterThanOrEqual(2);
  await expect(bar).toHaveAttribute("aria-valuetext", /\d+\s?% \(.* av .*\)/);
  // Knappene beholder fokusmål mens installasjonen pågår, men gjør ingenting.
  await expect(install(page)).toHaveAttribute("aria-disabled", "true");
  await expect(status(page)).toHaveText("Oppdateringen er installert. Appen starter på nytt …", { timeout: 8000 });
});

test("feil (frakoblet): forklaring med hva brukeren kan gjøre, og ny sjekk er mulig", async ({ page }) => {
  await open(page, "#/settings", "nb", 0, "&mockUpdate=error");
  await check(page).click();
  await expect(status(page)).toContainText("Sjekk internettforbindelsen og prøv igjen senere");
  await expect(status(page)).toContainText("bildene dine er ikke berørt");
  await expect(install(page)).toHaveCount(0);
  await check(page).click();
  await expect(status(page)).toContainText("Kunne ikke sjekke etter oppdatering");
  await go(page, "#/");
  await expect(page.getByText(/Ny versjon .* er tilgjengelig/)).toHaveCount(0);
});

test("skanning pågår: installasjon er aria-disabled med forklaring, og åpnes når skannet pauses", async ({ page }) => {
  test.setTimeout(90_000);
  await open(page, "#/", "nb", 0, "&mockUpdate=available&mockScanPolls=1000");
  await page.getByRole("button", { name: "Start skanning" }).click();
  await page.getByRole("button", { name: "Pause" }).waitFor();
  await go(page, "#/settings");
  await check(page).click();
  await expect(status(page)).toHaveText("Ny versjon 0.1.99 er tilgjengelig.");
  const btn = install(page);
  await expect(btn).toHaveAttribute("aria-disabled", "true");
  await expect(btn).toHaveAttribute("aria-describedby", "update-blocked");
  await expect(btn).toHaveAccessibleDescription(/Du kan ikke installere mens en skanning pågår/);
  await expect(page.locator("#update-blocked")).toContainText("Pause eller avbryt skanningen på Hjem");
  // Tastaturfokus når knappen (aria-disabled, ikke disabled), og klikk/Enter gjør ingenting.
  await btn.focus();
  await expect(btn).toBeFocused();
  await page.keyboard.press("Enter");
  await btn.click({ force: true }); // Playwright regner aria-disabled som deaktivert; force simulerer et museklikk likevel
  await page.waitForTimeout(300);
  await expect(status(page)).toHaveText("Ny versjon 0.1.99 er tilgjengelig.");
  await expect(section(page).getByRole("progressbar")).toHaveCount(0);
  // Pause skannet på Hjem; Innstillinger leser skannstatus på nytt og åpner knappen.
  await go(page, "#/");
  await page.getByRole("button", { name: "Pause" }).click();
  await go(page, "#/settings");
  await check(page).click();
  await expect(install(page)).not.toHaveAttribute("aria-disabled", "true", { timeout: 6000 });
  await expect(page.locator("#update-blocked")).toHaveCount(0);
  await install(page).click();
  await expect(status(page)).toHaveText("Oppdateringen er installert. Appen starter på nytt …");
});

test("auto på: stille sjekk ved oppstart gir banner på Hjem med lenke til Innstillinger, uten installasjon", async ({ page }) => {
  await open(page, "#/", "nb", 0, "&mockUpdate=available&mockAuto=1");
  const banner = page.locator(".banner").filter({ hasText: "Ny versjon 0.1.99 er tilgjengelig." });
  await expect(banner).toBeVisible();
  await expect(banner.getByRole("link", { name: "Innstillinger" })).toHaveAttribute("href", "#/settings");
  await banner.getByRole("link", { name: "Innstillinger" }).click();
  await expect(page.getByRole("heading", { name: "Innstillinger", level: 1 })).toBeVisible();
  await expect(page.getByRole("switch", { name: "Sjekk automatisk ved oppstart" })).toBeChecked();
  await expect(section(page).getByRole("progressbar")).toHaveCount(0); // aldri automatisk installasjon
  await expect(install(page)).toHaveCount(0); // ikke engang tilbudt før brukeren selv har sjekket
});

test("auto på og frakoblet: oppstart er stille (ingen banner, ingen feilmelding)", async ({ page }) => {
  await open(page, "#/", "nb", 0, "&mockUpdate=error&mockAuto=1");
  await page.waitForTimeout(400);
  await expect(page.getByText(/Ny versjon|Kunne ikke sjekke/)).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("tastaturflyt: Tab til knapp, Enter sjekker, Tab til Installer, Mellomrom slår bryteren på", async ({ page }) => {
  await open(page, "#/settings", "nb", 0, "&mockUpdate=available");
  await check(page).focus();
  await page.keyboard.press("Enter");
  await expect(status(page)).toHaveText("Ny versjon 0.1.99 er tilgjengelig.");
  await expect(check(page)).toBeFocused(); // fokus blir stående
  await page.keyboard.press("Tab");
  await expect(install(page)).toBeFocused();
  await page.keyboard.press("Tab");
  const auto = page.getByRole("switch", { name: "Sjekk automatisk ved oppstart" });
  await expect(auto).toBeFocused();
  await page.keyboard.press("Space");
  await expect(auto).toBeChecked();
  await page.keyboard.press("Space");
  await expect(auto).not.toBeChecked();
  await page.keyboard.press("Shift+Tab");
  await expect(install(page)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(status(page)).toHaveText("Oppdateringen er installert. Appen starter på nytt …");
  await expect(install(page)).toBeFocused(); // knappen forsvinner ikke under installasjon
});

test("ingen forespørsler utenfor appens opprinnelse, heller ikke når oppdateringsknappene brukes", async ({ page, baseURL }) => {
  const origin = new URL(baseURL ?? "http://localhost:1420").origin;
  const outside: string[] = [];
  await page.route((u) => !/^(data:|blob:|about:|ws:\/\/localhost)/.test(u.toString()) && !u.toString().startsWith(origin), (route) => { outside.push(route.request().url()); return route.abort(); });
  await open(page, "#/settings", "nb", 0, "&mockUpdate=available");
  await check(page).click();
  await install(page).click();
  await expect(status(page)).toContainText("starter på nytt");
  expect(outside).toEqual([]);
});

test("engelsk: tekster og live region", async ({ page }) => {
  await open(page, "#/settings", "en", 0, "&mockUpdate=available");
  await page.getByRole("button", { name: "Check for update" }).click();
  await expect(page.locator(".update .status")).toHaveText("Version 0.1.99 is available.");
  await expect(page.getByRole("button", { name: "Install and restart" })).toBeVisible();
  await expect(page.getByRole("switch", { name: "Check automatically at startup" })).toBeVisible();
});

test("pseudo-lokale (+30 %) på smal skjerm og 200 %: ingen avkutting i Oppdateringer", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 740 });
  await page.addInitScript(() => { document.addEventListener("DOMContentLoaded", () => { document.documentElement.style.fontSize = "200%"; }); });
  await open(page, "#/settings", "pseudo", 0, "&mockUpdate=available&mockScanPolls=1000");
  await page.locator(".update .row button").first().click();
  await page.locator(".update .notes").waitFor();
  const problems = await page.evaluate(() => {
    const out: string[] = [];
    const root = document.querySelector(".update");
    if (!root) return ["mangler .update"];
    for (const el of Array.from(root.querySelectorAll<HTMLElement>("*"))) {
      const cs = getComputedStyle(el);
      if (el.closest(".sr-only") || !el.getBoundingClientRect().width) continue;
      if (el.scrollWidth > el.clientWidth + 1 && cs.overflowX !== "visible" && el.tagName !== "svg") out.push(`${el.tagName}.${el.className} avkuttet bredde`);
      if (cs.textOverflow === "ellipsis") out.push(`${el.tagName}.${el.className} ellipse`);
      if (el.tagName === "BUTTON" && el.scrollHeight > el.clientHeight + 1) out.push(`knapp avkuttet høyde`);
    }
    if (document.documentElement.scrollWidth > document.documentElement.clientWidth + 1) out.push("horisontal rulling på siden");
    return out;
  });
  expect(problems, problems.join("\n")).toEqual([]);
});

const VIEWPORTS = { bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } } as const;
for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`axe oppdateringer ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      // Hjem med banner
      await open(page, "#/", "nb", 0, "&mockUpdate=slow&mockAuto=1&mockScanPolls=1000");
      await expect(page.locator(".banner").filter({ hasText: "Ny versjon" })).toBeVisible();
      await axe(page, "hjem med oppdateringsbanner");
      await page.getByRole("button", { name: "Start skanning" }).click();
      await page.getByRole("button", { name: "Pause" }).waitFor();
      // Innstillinger: hvile, sjekker, ny versjon blokkert, ny versjon åpen, installerer, feil
      await go(page, "#/settings");
      await axe(page, "innstillinger hvile");
      await check(page).click();
      await expect(status(page)).toHaveText("Ny versjon 0.1.99 er tilgjengelig.");
      await axe(page, "ny versjon, installasjon blokkert av skanning");
      await go(page, "#/");
      await page.getByRole("button", { name: "Pause" }).click();
      await go(page, "#/settings");
      await check(page).click();
      await expect(install(page)).not.toHaveAttribute("aria-disabled", "true", { timeout: 6000 });
      await axe(page, "ny versjon, klar til installasjon");
      await install(page).click();
      await expect(section(page).getByRole("progressbar")).toBeVisible();
      await axe(page, "installerer med fremdrift");
      await expect(status(page)).toHaveText("Oppdateringen er installert. Appen starter på nytt …", { timeout: 8000 });
      await axe(page, "starter på nytt");
      await open(page, "#/settings", "nb", 0, "&mockUpdate=error");
      await check(page).click();
      await expect(status(page)).toContainText("Kunne ikke sjekke");
      await axe(page, "feil");
      await open(page, "#/settings", "nb", 0, "&mockUpdate=none");
      await check(page).click();
      await expect(status(page)).toHaveText("Du har nyeste versjon.");
      await axe(page, "ingen ny versjon");
    });
  }
}

// ---------- Skjermbilder (bare med SHOTS=1): ett per hovedtilstand, JPEG, til sammen under 300 KB ----------
const OUT = process.env.SHOTS_DIR ?? "../docs/screens/m9";
test.describe("skjermbilder", () => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  test.use({ viewport: { width: 720, height: 780 } });
  async function shot(page: Page, name: string): Promise<void> {
    mkdirSync(OUT, { recursive: true });
    await page.locator(".update").scrollIntoViewIfNeeded();
    await page.waitForTimeout(250);
    await page.screenshot({ path: `${OUT}/${name}.jpg`, type: "jpeg", quality: 45 });
  }
  test("oppdateringer: hvile, ingen ny, ny versjon, blokkert, installerer, feil", async ({ page }) => {
    test.setTimeout(120_000);
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
    await open(page, "#/settings", "nb", 0, "&mockUpdate=none");
    await shot(page, "oppdateringer-hvile");
    await check(page).click();
    await expect(status(page)).toHaveText("Du har nyeste versjon.");
    await shot(page, "oppdateringer-ingen-ny");
    await open(page, "#/", "nb", 0, "&mockUpdate=slow&mockScanPolls=1000");
    await page.getByRole("button", { name: "Start skanning" }).click();
    await page.getByRole("button", { name: "Pause" }).waitFor();
    await go(page, "#/settings");
    await check(page).click();
    await expect(page.locator("#update-blocked")).toBeVisible();
    await shot(page, "oppdateringer-blokkert-av-skann");
    await go(page, "#/");
    await page.getByRole("button", { name: "Pause" }).click();
    await go(page, "#/settings");
    await check(page).click();
    await expect(install(page)).not.toHaveAttribute("aria-disabled", "true", { timeout: 6000 });
    await shot(page, "oppdateringer-ny-versjon");
    await install(page).click();
    await expect(section(page).getByRole("progressbar")).toHaveAttribute("aria-valuenow", /^[3-9]\d$/, { timeout: 6000 });
    await shot(page, "oppdateringer-installerer");
    await open(page, "#/settings", "nb", 0, "&mockUpdate=error");
    await check(page).click();
    await expect(status(page)).toContainText("Kunne ikke sjekke");
    await shot(page, "oppdateringer-feil");
    await open(page, "#/", "nb", 0, "&mockUpdate=available&mockAuto=1");
    await expect(page.locator(".banner").filter({ hasText: "Ny versjon" })).toBeVisible();
    await page.screenshot({ path: `${OUT}/hjem-oppdateringsbanner.jpg`, type: "jpeg", quality: 45 });
  });
});
