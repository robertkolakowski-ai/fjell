// Regler, sensitivitet og beskyttelser (V1) mot MockBridge: regelflyt med forhåndsvisning før lagring, sletting med Angre,
// tastatur og knapp gir samme handling, feilkoder som tekst, axe (lys/mørk × bred/smal, 0 brudd) og pseudo-språk uten avkutting.
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
    return r.violations.map((v: { id: string; nodes: { target: unknown[]; failureSummary?: string }[] }) =>
      ({ id: v.id, nodes: v.nodes.slice(0, 3).map((n) => `${n.target.join(" ")} :: ${n.failureSummary?.split("\n")[1] ?? ""}`) }));
  }, TAGS);
  expect(res, `axe-brudd: ${where}`).toEqual([]);
}

/** Ingen sidelengs rulling og ingen avkuttet tekst i nærmeste synlige elementer. */
async function noClipping(page: Page, where: string): Promise<void> {
  const bad = await page.evaluate(() => {
    const out: string[] = [];
    const root = document.documentElement;
    if (root.scrollWidth > root.clientWidth + 1) out.push(`sidelengs rulling ${root.scrollWidth}>${root.clientWidth}`);
    for (const el of document.querySelectorAll<HTMLElement>("main *, dialog[open] *")) {
      if (el.scrollWidth > el.clientWidth + 1 && !["inline", "contents"].includes(getComputedStyle(el).display)
        && !["SELECT", "INPUT", "svg", "path"].includes(el.tagName) && getComputedStyle(el).overflowX === "visible" && el.clientWidth > 0) {
        out.push(`${el.tagName}.${el.className} bredde ${el.scrollWidth}>${el.clientWidth}`);
      }
    }
    return out;
  });
  expect(bad, `avkutting: ${where}`).toEqual([]);
}

const VIEWPORTS = { bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } } as const;

test("regler: forhåndsvisning før lagring, opprett, av/på og slett med Angre", async ({ page }) => {
  await open(page, "#/rules");
  await expect(page.getByRole("heading", { level: 1, name: "Regler" })).toBeVisible();
  await expect(page.getByText("En regel flytter, sletter eller godtar aldri noe selv")).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Gamle skjermbilder" })).toBeVisible();
  await expect(page.getByText("Foreslår bilder der alle disse stemmer: Er et skjermbilde; Eldre enn 90 dager.")).toBeVisible();

  // Ny regel: Lagre er sperret til forhåndsvisning er kjørt.
  await page.getByRole("button", { name: "Ny regel" }).first().click();
  const dlg = page.getByRole("dialog", { name: "Ny regel" });
  await expect(dlg).toBeVisible();
  await expect(dlg.getByLabel("Navn")).toBeFocused();
  const save = dlg.getByRole("button", { name: "Lagre regel" });
  await expect(save).toHaveAttribute("aria-disabled", "true");
  await expect(dlg.getByText("Forhåndsvis treffene før du lagrer.")).toBeVisible();
  await save.click({ force: true }); // sperret: ingenting skjer
  await expect(dlg).toBeVisible();

  // Feilkoder som tekst: tomt vilkår gir invalid_condition.
  await dlg.getByLabel("Type for vilkår 1").selectOption("older_than_days");
  await dlg.getByRole("button", { name: "Forhåndsvis treff" }).click();
  await expect(dlg.getByRole("alert")).toHaveText("Vilkåret er ugyldig.");
  await dlg.getByLabel("Antall dager").fill("30");
  await dlg.getByRole("button", { name: "Forhåndsvis treff" }).click();
  await expect(dlg.getByText(/Treffer \d+ bilde/).first()).toBeVisible();
  await expect(save).not.toHaveAttribute("aria-disabled", "true");

  // Endring av vilkår gjør forhåndsvisningen utdatert og sperrer Lagre igjen.
  await dlg.getByLabel("Antall dager").fill("31");
  await expect(save).toHaveAttribute("aria-disabled", "true");
  await expect(dlg.getByText("Vilkårene er endret. Forhåndsvis på nytt før du lagrer.").first()).toBeVisible();

  // Enter i feltet gir samme handling som knappen: forhåndsvis, så lagre. Navn mangler først: feilkode som tekst.
  await dlg.getByLabel("Antall dager").press("Enter");
  await expect(dlg.getByText(/Treffer \d+ bilde/).first()).toBeVisible();
  await dlg.getByLabel("Antall dager").press("Enter");
  await expect(dlg.getByRole("alert")).toHaveText("Regelen trenger et navn.");
  await dlg.getByLabel("Navn").fill("Gamle bilder");
  await save.click();
  await expect(dlg).toBeHidden();
  await expect(page.getByRole("heading", { level: 2, name: "Gamle bilder" })).toBeVisible();
  await expect(page.getByText("Foreslår bilder der alle disse stemmer: Eldre enn 31 dager.")).toBeVisible();
  await expect(page.getByText("Regelen «Gamle bilder» er lagret.").first()).toBeVisible();

  // Av/på med tastatur (mellomrom) og mus gir samme handling.
  const sw = page.getByRole("switch", { name: "Aktiv: Gamle bilder" });
  await expect(sw).toBeChecked();
  await sw.focus();
  await page.keyboard.press("Space");
  await expect(sw).not.toBeChecked();
  await expect(page.getByText("Regelen «Gamle bilder» er slått av.").first()).toBeVisible();
  await sw.click();
  await expect(sw).toBeChecked();

  // Slett med Angre.
  await page.getByRole("button", { name: "Slett regel Gamle bilder" }).click();
  await expect(page.getByRole("heading", { level: 2, name: "Gamle bilder" })).toHaveCount(0);
  const toast = page.getByRole("status").filter({ hasText: "er slettet. Bildene er urørt." });
  await expect(toast).toBeVisible();
  await toast.getByRole("button", { name: "Angre" }).click();
  await expect(page.getByRole("heading", { level: 2, name: "Gamle bilder" })).toBeVisible();
});

test("regler: rediger en eksisterende regel og Esc lukker uten endring", async ({ page }) => {
  await open(page, "#/rules");
  await page.getByRole("button", { name: "Rediger regelen Store filer" }).click();
  const dlg = page.getByRole("dialog", { name: "Rediger regel" });
  await expect(dlg.getByLabel("Navn")).toHaveValue("Store filer");
  await expect(dlg.getByLabel("Størrelse")).toHaveValue("20");
  await expect(dlg.getByText(/Treffer \d+ bilde/).first()).toBeVisible(); // forhåndsvises ved åpning
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(page.getByRole("button", { name: "Rediger regelen Store filer" })).toBeFocused();
  await expect(page.getByRole("heading", { level: 2, name: "Store filer" })).toBeVisible();
});

test("regler: alle vilkårstyper kan velges", async ({ page }) => {
  await open(page, "#/rules");
  await page.getByRole("button", { name: "Ny regel" }).first().click();
  const dlg = page.getByRole("dialog", { name: "Ny regel" });
  const sel = dlg.getByLabel("Type for vilkår 1");
  const types: [string, string | null][] = [
    ["is_screenshot", null], ["is_blurry", null], ["is_exact_duplicate", null], ["older_than_days", "Antall dager"],
    ["larger_than_bytes", "Størrelse"], ["smaller_than_bytes", "Størrelse"], ["in_album", "Albumnummer"], ["in_folder", "Mappesti"], ["mime", "Filtype, for eksempel image/png"],
  ];
  for (const [value, field] of types) {
    await sel.selectOption(value);
    if (field) await expect(dlg.getByLabel(field)).toBeVisible();
  }
  await dlg.getByRole("button", { name: "Legg til vilkår" }).click();
  await expect(dlg.getByLabel("Type for vilkår 2")).toBeFocused();
  await dlg.getByRole("button", { name: "Fjern vilkår 2" }).click();
  await expect(dlg.getByLabel("Type for vilkår 2")).toHaveCount(0);
});

test("innstillinger: sensitivitet per forslagstype med terskler som info", async ({ page }) => {
  await open(page, "#/settings");
  const card = page.getByRole("group", { name: "Hvor mange forslag" });
  await expect(card.getByText("Appen foreslår bare.")).toBeVisible();
  await expect(card.getByText("Største forskjell mellom like bilder: 6")).toBeVisible();
  await expect(card.getByRole("textbox")).toHaveCount(0); // terskler er ikke redigerbare
  const group = card.getByRole("radiogroup", { name: "Like bilder" });
  await expect(group.getByRole("radio", { name: "Normal" })).toBeChecked();
  await group.getByRole("radio", { name: "Flere forslag, mindre sikre" }).check();
  await expect(card.getByText("Største forskjell mellom like bilder: 8")).toBeVisible();
  await expect(group.getByRole("radio", { name: "Flere forslag, mindre sikre" })).toBeChecked();
  await expect(card.getByRole("radiogroup")).toHaveCount(3);
  // Piltaster gir samme valg som mus.
  await group.getByRole("radio", { name: "Flere forslag, mindre sikre" }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(group.getByRole("radio", { name: "Normal" })).toBeChecked();
  await expect(card.getByText("Største forskjell mellom like bilder: 6")).toBeVisible();
});

test("innstillinger: beskyttelser, fjern med bekreftelse og Angre, beskytt mappe", async ({ page }) => {
  await open(page, "#/settings");
  const card = page.getByRole("group", { name: "Beskyttelser" });
  await expect(card.getByText("Du har ikke beskyttet noe ennå.")).toBeVisible();

  await card.getByRole("button", { name: "Beskytt en mappe" }).click();
  const dlg = page.getByRole("dialog", { name: "Beskytt en mappe" });
  await expect(dlg.getByRole("button", { name: "Beskytt", exact: true })).toBeDisabled();
  await dlg.getByLabel("Mappens sti").fill("D:\\Bilder\\Ferie");
  await dlg.getByRole("button", { name: "Beskytt", exact: true }).click();
  await expect(dlg).toBeHidden();
  await expect(card.getByText("D:\\Bilder\\Ferie", { exact: true })).toBeVisible();
  await expect(card.getByText("Mappen D:\\Bilder\\Ferie er beskyttet.").first()).toBeVisible();

  // Fjern krever bekreftelse; Avbryt endrer ingenting.
  await card.getByRole("button", { name: /Fjern beskyttelse for Mappe D:/ }).click();
  const confirm = page.getByRole("dialog", { name: "Fjerne beskyttelsen?" });
  await expect(confirm).toBeVisible();
  await confirm.getByRole("button", { name: "Avbryt" }).click();
  await expect(card.getByText("D:\\Bilder\\Ferie", { exact: true })).toBeVisible();

  await card.getByRole("button", { name: /Fjern beskyttelse for Mappe D:/ }).click();
  await confirm.getByRole("button", { name: "Fjern beskyttelse" }).click();
  await expect(card.getByText("Du har ikke beskyttet noe ennå.")).toBeVisible();
  await card.getByRole("button", { name: "Angre" }).click();
  await expect(card.getByText("D:\\Bilder\\Ferie", { exact: true })).toBeVisible();
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`axe: regler, sensitivitet og beskyttelser ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await open(page, "#/rules");
      await axe(page, "regler liste");
      await page.getByRole("button", { name: "Ny regel" }).first().click();
      await expect(page.getByRole("dialog", { name: "Ny regel" })).toBeVisible();
      await axe(page, "regelbygger ny");
      const dlg = page.getByRole("dialog", { name: "Ny regel" });
      await dlg.getByLabel("Type for vilkår 1").selectOption("larger_than_bytes");
      await dlg.getByLabel("Størrelse").fill("1");
      await dlg.getByRole("button", { name: "Forhåndsvis treff" }).click();
      await expect(dlg.getByText(/Treffer \d+ bilde/).first()).toBeVisible();
      await axe(page, "regelbygger med forhåndsvisning");
      await dlg.getByLabel("Størrelse").fill("2");
      await axe(page, "regelbygger utdatert forhåndsvisning");
      await page.keyboard.press("Escape");
      await page.getByRole("button", { name: "Slett regel Store filer" }).click();
      await expect(page.getByRole("button", { name: "Angre" })).toBeVisible();
      await axe(page, "regler med angre");

      await go(page, "#/rules");
      await go(page, "#/settings");
      await expect(page.getByRole("group", { name: "Beskyttelser" }).getByText("Du har ikke beskyttet noe ennå.")).toBeVisible();
      await axe(page, "innstillinger tom beskyttelse");
      const card = page.getByRole("group", { name: "Beskyttelser" });
      await card.getByRole("button", { name: "Beskytt en mappe" }).click();
      await axe(page, "beskytt mappe-dialog");
      await page.getByRole("dialog", { name: "Beskytt en mappe" }).getByLabel("Mappens sti").fill("D:\\Bilder");
      await page.getByRole("dialog", { name: "Beskytt en mappe" }).getByRole("button", { name: "Beskytt", exact: true }).click();
      await expect(card.getByText("D:\\Bilder", { exact: true })).toBeVisible();
      await axe(page, "innstillinger med beskyttelse");
      await card.getByRole("button", { name: /Fjern beskyttelse for/ }).click();
      await axe(page, "bekreft fjerning");
    });
  }
}

for (const lang of ["pseudo", "en"] as const) {
  for (const [vp, size] of Object.entries(VIEWPORTS)) {
    test(`${lang}: ingen avkutting eller sideskroll ${vp}`, async ({ page }) => {
      test.setTimeout(90_000);
      await page.setViewportSize(size);
      await open(page, "#/rules", lang);
      await noClipping(page, `${lang} regler ${vp}`);
      await page.locator("main .btn.primary").first().click();
      await expect(page.getByRole("dialog")).toBeVisible();
      await page.locator("dialog[open] .pv-head button").click();
      await page.waitForTimeout(200);
      await noClipping(page, `${lang} regelbygger ${vp}`);
      await page.keyboard.press("Escape");
      await go(page, "#/settings");
      await page.waitForTimeout(200);
      await noClipping(page, `${lang} innstillinger ${vp}`);
    });
  }
}
