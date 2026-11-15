// V1 i Windows høykontrast, emulert med forced-colors: active i Chromium (lys og mørk basis).
// Sjekker per V1-skjerm: knapper har kant, fokus har outline (box-shadow forsvinner i forced-colors), tilstand er ikke bare farge
// (ProtectToggle, bryter, valgt flis), tekst har ikke forced-color-adjust: none, og kontrasten i beregnede stiler holder 4,5:1.
// Dette er IKKE et ekte Windows-kontrasttema (Aquatic, Desert, Dusk, Night sky); de sjekkes manuelt (docs/v1/WINDOWS-V1.md del 4).
import { expect, test, type Page } from "@playwright/test";
import { go } from "./helpers";
import { kontrastBeregnet, layoutProblemer, scanned, tabGjennom, tabTil, tilToppen, V1_SKJERMER } from "./v1-win-felles";

async function kontroller(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    const navn = (e: Element) => `«${(e.getAttribute("aria-label") ?? e.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 30)}»`;
    // Knapper skal ha synlig kant: forced-colors fjerner bakgrunnsfarger, og kanten er det som skiller dem fra tekst.
    for (const b of Array.from(document.querySelectorAll<HTMLElement>("main button, header button, nav a"))) {
      if (!b.checkVisibility({ checkVisibilityCSS: true }) || b.closest(".sr-only") || b.closest("dialog:not([open])")) continue;
      const cs = getComputedStyle(b);
      const kant = cs.borderTopStyle !== "none" && parseFloat(cs.borderTopWidth) >= 1;
      const ramme = cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) >= 1;
      // Navigasjonslenker og rene tekstknapper («quiet») er unntatt når aria-current/understreking viser hvor man er.
      if (!kant && !ramme && !b.closest("nav") && !b.matches(".quiet, .link, .linklike")) out.push(`knapp uten kant: ${navn(b)}`);
    }
    // Ingen tekst med forced-color-adjust: none (da overstyrer designet brukerens kontrasttema).
    for (const el of Array.from(document.querySelectorAll<HTMLElement>("main *, nav *, header *"))) {
      if (el instanceof SVGElement || !el.checkVisibility({ checkVisibilityCSS: true })) continue;
      if (getComputedStyle(el).forcedColorAdjust === "none" && (el.textContent ?? "").trim()) out.push(`forced-color-adjust: none på tekst: ${el.tagName.toLowerCase()}.${el.className}`);
    }
    // Tilstand uten farge: beskyttelsesbryter har tilstandsord (På/Av) i tillegg til aria-pressed.
    for (const t of Array.from(document.querySelectorAll<HTMLElement>("button[aria-pressed]"))) {
      if (!t.checkVisibility({ checkVisibilityCSS: true })) continue;
      const ord = (t.textContent ?? "").trim();
      if (t.classList.contains("tile")) continue; // flis: egen sjekk under
      if (!/(^|\s)(På|Av|On|Off)(\s|$)/.test(ord) && !t.querySelector("svg")) out.push(`bryter uten tilstandsord eller ikon: ${navn(t)}`);
    }
    // Valgt flis: hake-ikon eller tykkere kant, ikke bare farge.
    for (const t of Array.from(document.querySelectorAll<HTMLElement>(".tile[aria-pressed=true]"))) {
      const ikon = !!t.querySelector("svg");
      const w = parseFloat(getComputedStyle(t).borderTopWidth);
      if (!ikon && w < 3) out.push("valgt flis er bare markert med farge");
    }
    // Systembrytere (input[role=switch], checkbox, radio) skal ha en tekstlig eller formmessig tilstand: label finnes.
    for (const i of Array.from(document.querySelectorAll<HTMLInputElement>("input[type=checkbox], input[type=radio]"))) {
      if (!i.checkVisibility({ checkVisibilityCSS: true }) || i.closest("dialog:not([open])")) continue;
      if (!i.labels || i.labels.length === 0) out.push(`avkrysning uten label: ${i.name || i.id}`);
    }
    return out;
  });
}

for (const scheme of ["light", "dark"] as const) {
  test(`høykontrast ${scheme}: hver V1-skjerm har kant, tilstand uten farge og kontrast`, async ({ page }) => {
    test.setTimeout(120_000);
    await page.emulateMedia({ forcedColors: "active", colorScheme: scheme, reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1280, height: 800 });
    await scanned(page, "#/");
    expect(await page.evaluate(() => matchMedia("(forced-colors: active)").matches)).toBe(true);
    const funn: string[] = [];
    for (const s of V1_SKJERMER) {
      await go(page, s.hash);
      await page.waitForTimeout(120);
      funn.push(...(await kontroller(page)).map((f) => `${s.navn}: ${f}`));
      funn.push(...(await kontrastBeregnet(page)).map((f) => `${s.navn}: kontrast ${f}`));
      funn.push(...(await layoutProblemer(page)).map((f) => `${s.navn}: ${f}`));
    }
    expect(funn, funn.join("\n")).toEqual([]);
  });

  test(`høykontrast ${scheme}: hvert fokusstopp har outline på alle V1-skjermer`, async ({ page }) => {
    test.setTimeout(120_000);
    await page.emulateMedia({ forcedColors: "active", colorScheme: scheme, reducedMotion: "reduce" });
    await scanned(page, "#/");
    const funn: string[] = [];
    for (const s of V1_SKJERMER) {
      await go(page, s.hash);
      await page.waitForTimeout(100);
      const stopp = await tabGjennom(page);
      expect(stopp.length, `${s.navn}: ingen fokusstopp`).toBeGreaterThan(5);
      for (const st of stopp) if (!st.outline) funn.push(`${s.navn}: «${st.navn}» (${st.tag}) har ingen outline i forced-colors`);
    }
    expect(funn, funn.join("\n")).toEqual([]);
  });
}

test("høykontrast: ProtectToggle viser På/Av i tekst og aria-pressed endres uten fargeavhengighet", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", colorScheme: "dark", reducedMotion: "reduce" });
  await scanned(page, "#/series/41");
  const bryter = page.getByRole("button", { name: "Beskytt bilde 1" });
  await expect(bryter).toHaveAttribute("aria-pressed", "false");
  await expect(bryter).toContainText("Av");
  await tilToppen(page);
  await tabTil(page, "Beskytt bilde 1");
  await expect(bryter).toBeFocused();
  await page.keyboard.press("Space");
  await expect(bryter).toHaveAttribute("aria-pressed", "true");
  await expect(bryter).toContainText("På");
  // Utseendet må endres i en annen egenskap enn farge alene (kant, ikon eller tekst): tekstinnholdet er endret, ikonet finnes.
  await expect(bryter.locator("svg")).toBeVisible();
  const kant = await bryter.evaluate((e) => ({ w: parseFloat(getComputedStyle(e).borderTopWidth), s: getComputedStyle(e).borderTopStyle }));
  expect(kant.s).not.toBe("none");
  expect(kant.w).toBeGreaterThanOrEqual(1);
});

test("høykontrast: bryteren på Regler og valgene i Innstillinger har label og synlig fokus", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", colorScheme: "light", reducedMotion: "reduce" });
  await scanned(page, "#/rules");
  const sw = page.getByRole("switch", { name: "Aktiv: Gamle skjermbilder" });
  await tilToppen(page);
  await tabTil(page, "Aktiv");
  await expect(sw).toBeFocused();
  const fokus = await sw.evaluate((e) => ({ s: getComputedStyle(e).outlineStyle, w: parseFloat(getComputedStyle(e).outlineWidth) }));
  expect(fokus.s).not.toBe("none");
  expect(fokus.w).toBeGreaterThanOrEqual(2);
  const før = await sw.isChecked();
  await page.keyboard.press("Space");
  expect(await sw.isChecked()).toBe(!før);
  await go(page, "#/settings");
  const gruppe = page.getByRole("group", { name: "Hvor mange forslag" });
  await expect(gruppe.getByRole("radiogroup")).toHaveCount(3);
  for (const r of await gruppe.getByRole("radio").all()) await expect(r).toHaveAccessibleName(/\S/);
});

test("høykontrast sammen med prefers-contrast: more: V1-skjermene har ingen layoutfeil", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", contrast: "more", colorScheme: "dark", reducedMotion: "reduce" });
  await scanned(page, "#/");
  const funn: string[] = [];
  for (const s of V1_SKJERMER) {
    await go(page, s.hash);
    await page.waitForTimeout(100);
    funn.push(...(await layoutProblemer(page)).map((f) => `${s.navn}: ${f}`));
  }
  expect(funn, funn.join("\n")).toEqual([]);
});

test("selvtest: kontrastkontrollen finner lav kontrast (testen er ikke tom)", async ({ page }) => {
  await scanned(page, "#/");
  await page.addStyleTag({ content: "h1 { color: #777 !important; background: #888 !important; }" });
  const funn = await kontrastBeregnet(page);
  expect(funn.join()).toContain("h1");
});
