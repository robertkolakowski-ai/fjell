// V1 tastatur-bare gjennomgang (ingen mus under selve flyten; skanningen som gir data startes med mus i `scanned`, det er oppsett).
// Hver V1-skjerm: alle kontroller kan nås med Tab, ingen fokusfelle, fokus er synlig, navn finnes. Flyter: Serie, Regler, Personer,
// Kortforklaring og Innstillinger kan fullføres og angres med Tab, Enter, Space, Esc og piler. Dialoger fanger og gir tilbake fokus.
// Chromium emulerer ikke Narrator eller NVDA: opplesning og skjermleserens egne taster står i docs/v1/WINDOWS-V1.md.
import { expect, test, type Page } from "@playwright/test";
import { go } from "./helpers";
import { fokuserbare, scanned, tabGjennom, tabTil, tilToppen, V1_SKJERMER } from "./v1-win-felles";

/** Navnet på elementet som har fokus nå (aria-label, label eller tekst). */
const fokusNavn = (page: Page) =>
  page.evaluate(() => {
    const a = document.activeElement as HTMLElement | null;
    if (!a) return "";
    const labels = a instanceof HTMLInputElement ? Array.from(a.labels ?? []).map((l) => l.textContent ?? "").join(" ") : "";
    return (a.getAttribute("aria-label") || labels || a.textContent || "").replace(/\s+/g, " ").trim();
  });

/** Fokus står innenfor `selector`. */
const fokusI = (page: Page, selector: string) => page.evaluate((s) => !!document.activeElement?.closest(s), selector);

test("Hopp til hovedinnhold er første stopp og flytter fokus til innholdet", async ({ page }) => {
  await scanned(page, "#/rules");
  await tilToppen(page);
  expect(await fokusNavn(page)).toBe("Hopp til hovedinnhold");
  await page.keyboard.press("Enter");
  // Fokus er i (eller på) hovedområdet; neste Tab går til første kontroll i innholdet, ikke tilbake til navigasjonen.
  expect(await fokusI(page, "main")).toBe(true);
  await page.keyboard.press("Tab");
  expect(await fokusI(page, "main")).toBe(true);
});

test("navigasjonen kan brukes med tastatur til Personer og Regler, og fokus havner på h1", async ({ page }) => {
  await scanned(page, "#/");
  const nav = page.getByRole("navigation", { name: "Hovednavigasjon" });
  await tilToppen(page);
  await tabTil(page, "Personer");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "Personer" })).toBeFocused();
  await expect(nav.getByRole("link", { name: /Personer/ })).toHaveAttribute("aria-current", "page");
  await tilToppen(page);
  await tabTil(page, "Regler");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "Regler" })).toBeFocused();
  await expect(page).toHaveTitle(/Regler/);
});

for (const s of V1_SKJERMER) {
  test(`Tab-gjennomgang: ${s.navn}: alt nåbart, ingen felle, navn og synlig fokus`, async ({ page }) => {
    test.setTimeout(60_000);
    await scanned(page, s.hash);
    await expect(page.getByRole("heading", { level: 1 }).first()).toHaveText(s.h1);
    const forventet = await fokuserbare(page);
    const stopp = await tabGjennom(page, 150);
    expect(stopp.length, "ingen fokusstopp").toBeGreaterThan(3);
    // Ingen felle: tastaturet kommer ut av dokumentet før grensen, og samme element står ikke to ganger rett etter hverandre.
    expect(stopp.length, "fokusfelle: grensen på 150 stopp nådd").toBeLessThan(150);
    for (let i = 1; i < stopp.length; i++) expect(stopp[i].navn === stopp[i - 1].navn && stopp[i].tag === stopp[i - 1].tag && !stopp[i].radioGruppe, `to like stopp etter hverandre: ${stopp[i].navn}`).toBe(false);
    // Alt som er synlig og fokuserbart nås; radioknapper per gruppe.
    const nådd = new Set(stopp.map((x) => x.navn));
    const radioNådd = new Set(stopp.map((x) => x.radioGruppe).filter(Boolean));
    const mangler = forventet.filter((f) => (f.startsWith("radio:") ? !radioNådd.has(f.slice(6)) : !nådd.has(f)));
    expect(mangler, `ikke nådd med Tab: ${mangler.join(" | ")}`).toEqual([]);
    // Navn og synlig fokus.
    const uten = stopp.filter((x) => x.navn === "(uten navn)").map((x) => x.tag);
    expect(uten, "fokusstopp uten navn").toEqual([]);
    const usynlig = stopp.filter((x) => !x.outline && !x.skygge).map((x) => `${x.tag} «${x.navn}»`);
    expect(usynlig, "fokusstopp uten synlig indikator").toEqual([]);
    expect(stopp.filter((x) => !x.innenforBredde).map((x) => x.navn), "fokus utenfor vinduet").toEqual([]);
  });
}

test("Serie: beskytt, gjør til beste, flytt og angre kan gjøres med tastatur alene", async ({ page }) => {
  await scanned(page, "#/series/41");
  const items = page.locator("ul.items > li.item");
  await expect(items).toHaveCount(4);
  const førsteFør = await items.first().getAttribute("data-asset");
  // Beskytt bilde 2 med Space (ProtectToggle er en knapp med aria-pressed).
  await tilToppen(page);
  await tabTil(page, "Beskytt bilde 2");
  const bryter = page.getByRole("button", { name: "Beskytt bilde 2" });
  await expect(bryter).toBeFocused();
  await page.keyboard.press("Space");
  await expect(bryter).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Enter");
  await expect(bryter).toHaveAttribute("aria-pressed", "false");
  // Gjør bilde 3 til beste med Enter; fokus ender på et element i serien (ikke body).
  await tabTil(page, "Gjør dette til beste");
  await page.keyboard.press("Enter");
  await expect(items.first()).not.toHaveAttribute("data-asset", førsteFør!);
  expect(await page.evaluate(() => document.activeElement === document.body), "fokus mistet etter «Gjør dette til beste»").toBe(false);
  // Flytt resten, så angre: begge med tastatur.
  // Beskyttelse av bilde 2 tok det ut av utvalget, og å fjerne beskyttelsen velger det ikke på nytt (trygg retning): 2 av 3 gjenstår.
  await tabTil(page, "Flytt 2 til papirkurv");
  await page.keyboard.press("Enter");
  await expect(page.getByText("Serien er ferdig. 2 bilder i papirkurven.")).toBeVisible();
  await expect(page.locator(".done")).toBeFocused();
  await tabTil(page, "Angre", { eksakt: true });
  await page.keyboard.press("Enter");
  await expect(items).toHaveCount(4);
});

test("Regler: ny regel, bryter, sletting og angre med tastatur; dialogen fanger og returnerer fokus", async ({ page }) => {
  await scanned(page, "#/rules");
  await tilToppen(page);
  await tabTil(page, "Ny regel", { eksakt: true });
  const ny = page.getByRole("button", { name: "Ny regel" }).first();
  await expect(ny).toBeFocused();
  await page.keyboard.press("Enter");
  const dlg = page.getByRole("dialog", { name: "Ny regel" });
  await expect(dlg).toBeVisible();
  await expect(dlg.getByLabel("Navn")).toBeFocused();
  // Fokusfelle i modal dialog er riktig: Tab og Shift+Tab forlater aldri dialogen.
  // Fokus kan gå til nettleserens egen verktøylinje (activeElement = body) mellom siste og første kontroll; aldri til siden bak.
  const innenfor = async () => (await fokusI(page, "dialog[open]")) || (await page.evaluate(() => document.activeElement === document.body));
  for (let i = 0; i < 14; i++) {
    await page.keyboard.press("Tab");
    expect(await innenfor(), `Tab ${i + 1} forlot dialogen`).toBe(true);
  }
  for (let i = 0; i < 14; i++) {
    await page.keyboard.press("Shift+Tab");
    expect(await innenfor(), `Shift+Tab ${i + 1} forlot dialogen`).toBe(true);
  }
  await page.evaluate(() => (document.querySelector<HTMLElement>("dialog[open] input") ?? document.body).focus());
  // Esc lukker og fokus går tilbake til knappen som åpnet den.
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(ny).toBeFocused();
  // Ny, og denne gangen fullfør: navn, forhåndsvis, lagre, alt med tastatur. Kort pause: Playwright trykker raskere enn et menneske,
  // og Enter rett i det fokus returneres kan treffe før dialogens lukketilstand er ferdig (observert som sporadisk feil).
  await page.waitForTimeout(150);
  await page.keyboard.press("Enter");
  await expect(dlg.getByLabel("Navn")).toBeFocused();
  await page.keyboard.type("Tastaturregel");
  await tabTil(page, "Forhåndsvis treff");
  await page.keyboard.press("Enter");
  await expect(dlg.getByText(/Treffer \d+ bilde/).first()).toBeVisible();
  await tabTil(page, "Lagre regel");
  await page.keyboard.press("Enter");
  await expect(dlg).toBeHidden();
  await expect(page.getByRole("heading", { level: 2, name: "Tastaturregel" })).toBeVisible();
  expect(await page.evaluate(() => document.activeElement === document.body), "fokus mistet etter lagring").toBe(false);
  // Bryter med Space, deretter slett og angre fra meldingen.
  await tilToppen(page);
  await tabTil(page, "Aktiv: Tastaturregel");
  const sw = page.getByRole("switch", { name: "Aktiv: Tastaturregel" });
  await expect(sw).toBeChecked();
  await page.keyboard.press("Space");
  await expect(sw).not.toBeChecked();
  await tabTil(page, "Slett regel Tastaturregel");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 2, name: "Tastaturregel" })).toHaveCount(0);
  const melding = page.getByRole("status").filter({ hasText: "er slettet. Bildene er urørt." });
  await expect(melding).toBeVisible();
  await tabTil(page, "Angre", { eksakt: true });
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 2, name: "Tastaturregel" })).toBeVisible();
});

test("Personer: samtykke med Esc, slå på, gi navn og angre med tastatur", async ({ page }) => {
  await scanned(page, "#/people");
  await tilToppen(page);
  await tabTil(page, "Slå på ansiktsindeks");
  const slåPå = page.getByRole("button", { name: "Slå på ansiktsindeks" });
  await page.keyboard.press("Enter");
  const dlg = page.getByRole("dialog", { name: "Slå på ansiktsindeks?" });
  await expect(dlg).toBeVisible();
  await expect(dlg.getByRole("button", { name: "Avbryt" })).toBeFocused(); // trygg standard i et samtykke
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(slåPå).toBeFocused();
  await expect(page.getByRole("list", { name: "Personer" })).toHaveCount(0); // Esc slo ikke på noe
  await page.waitForTimeout(150); // som i Regler: ikke raskere enn et menneske rett etter at fokus er returnert
  await page.keyboard.press("Enter");
  await tabTil(page, "Slå på", { eksakt: true });
  await page.keyboard.press("Enter");
  await expect(page.getByRole("list", { name: "Personer" }).getByRole("listitem")).toHaveCount(4);
  await expect(page.getByRole("heading", { level: 1 })).toBeFocused();
  // Gi navn til en uten navn: knapp, tekst, Enter.
  await tabTil(page, "Gi navn til Uten navn");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("textbox", { name: "Navn" })).toBeFocused();
  await page.keyboard.type("Eva");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 2, name: "Eva" })).toBeVisible();
  const melding = page.getByRole("status").filter({ hasText: "Navnet på Eva er endret." });
  await expect(melding).toBeVisible();
  await tabTil(page, "Angre", { eksakt: true });
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 2, name: "Eva" })).toHaveCount(0);
});

test("Kortforklaring: serie og regel kan avgjøres og angres med knapper og taster, og F1 åpner og lukker hjelpen", async ({ page }) => {
  await scanned(page, "#/cards/series");
  const kort = page.getByRole("group", { name: /^Forslag 1 av 6/ });
  await expect(kort).toBeVisible();
  await expect(kort).toHaveAccessibleName(/Bilde 2 av 4 i serien/); // forklaringen er en del av kortets navn
  await tilToppen(page);
  await tabTil(page, "Forslag 1 av 6");
  await expect(kort).toBeFocused();
  // Tast og knapp har samme virkning: ArrowRight hopper over (kortet byttes, telleren står), ArrowLeft beholder (telleren går fram).
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("group", { name: /^Forslag 1 av 6.*Bilde 3 av 4 i serien/ })).toBeVisible();
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByRole("group", { name: /^Forslag 2 av 6/ })).toBeVisible();
  await expect(page.getByText("5 igjen").first()).toBeVisible();
  await page.keyboard.press("Control+z");
  await expect(page.getByRole("group", { name: /^Forslag 1 av 6/ })).toBeVisible();
  await expect(page.getByText("6 igjen").first()).toBeVisible();
  await page.keyboard.press("F1");
  const hjelp = page.getByRole("dialog");
  await expect(hjelp).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(hjelp).toBeHidden();
  expect(await page.evaluate(() => document.activeElement === document.body), "fokus mistet etter F1/Esc").toBe(false);
  await tabTil(page, "Behold", { eksakt: true });
  await page.keyboard.press("Enter");
  await expect(page.getByText("5 igjen").first()).toBeVisible();
  await tabTil(page, "Angre", { eksakt: true });
  await page.keyboard.press("Enter");
  await expect(page.getByText("6 igjen").first()).toBeVisible();
  // Regelkort (via Hjem): forklaringen nevner regelen.
  await go(page, "#/");
  await go(page, "#/cards/rule");
  await expect(page.getByRole("group", { name: /^Forslag 1 av 6/ })).toHaveAccessibleName(/Regel: /);
});

// Rettet i integrasjonen: App.svelte nøkler <Cards> på kind, så hash-skifte mellom korttyper bytter bunke.
test("Kortforklaring: hash-skifte mellom korttyper bytter bunke", async ({ page }) => {
  await scanned(page, "#/cards/series");
  await go(page, "#/cards/rule");
  await expect(page.getByRole("group", { name: /^Forslag 1 av 6/ })).toHaveAccessibleName(/Regel: /, { timeout: 2000 });
});

test("Innstillinger: Ansikter og personer med tastatur (samtykke avbrytes med Esc, valget består)", async ({ page }) => {
  await scanned(page, "#/settings");
  const gruppe = page.getByRole("group", { name: "Ansikter og personer" });
  await expect(gruppe.getByRole("radio", { name: "Av" })).toBeChecked();
  await tilToppen(page);
  await tabTil(page, "Av", { eksakt: true, maks: 120 });
  await expect(gruppe.getByRole("radio", { name: "Av" })).toBeFocused();
  await page.keyboard.press("ArrowRight"); // radiogruppe: pil velger «På», som åpner samtykket
  const dlg = page.getByRole("dialog", { name: "Slå på ansiktsindeks?" });
  await expect(dlg).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(gruppe.getByRole("radio", { name: "Av" })).toBeChecked(); // avbrutt: ingenting slått på
  expect(await page.evaluate(() => document.activeElement === document.body), "fokus mistet etter Esc").toBe(false);
});

test("Innstillinger: sensitivitet og beskyttelser nås med piler og Tab; tilstand uten mus", async ({ page }) => {
  await scanned(page, "#/settings");
  const kort = page.getByRole("group", { name: "Hvor mange forslag" });
  const like = kort.getByRole("radiogroup", { name: "Like bilder" });
  await tilToppen(page);
  await tabTil(page, "Normal", { maks: 120 });
  // Første radiogruppe i kortet kan være en annen type; gå til «Like bilder» med Tab til en radio i den har fokus.
  for (let i = 0; i < 20 && !(await like.getByRole("radio", { name: "Normal" }).evaluate((e) => e === document.activeElement)); i++) await page.keyboard.press("Tab");
  await expect(like.getByRole("radio", { name: "Normal" })).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(like.getByRole("radio", { name: "Flere forslag, mindre sikre" })).toBeChecked();
  await page.keyboard.press("ArrowLeft");
  await expect(like.getByRole("radio", { name: "Normal" })).toBeChecked();
  await tabTil(page, "Beskytt en mappe");
  await page.keyboard.press("Enter");
  const dlg = page.getByRole("dialog", { name: "Beskytt en mappe" });
  await expect(dlg).toBeVisible();
  await expect(dlg.getByRole("button", { name: "Beskytt", exact: true })).toBeDisabled(); // tom sti: ingenting å beskytte
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(page.getByRole("button", { name: "Beskytt en mappe" })).toBeFocused();
});
