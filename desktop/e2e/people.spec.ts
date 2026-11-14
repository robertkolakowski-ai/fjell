// Personer og ansiktsindeks (V1): flyt mot MockBridge (tom tilstand, samtykke, navn, sammenslåing med angre, beskyttelse,
// slett ansiktsindeks) og axe 0 brudd (lys/mørk × bred/smal). Pseudo-lokale uten avkutting. Tastatur og knapp gir samme handling.
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

/** Slår på indeksen fra tom tilstand via samtykkedialogen. */
async function enable(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Slå på ansiktsindeks" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Slå på", exact: true }).click();
  await expect(page.getByRole("heading", { level: 2, name: "Kari" })).toBeVisible();
}

test("tom tilstand: forklarer, er on-device og slår aldri på av seg selv", async ({ page }) => {
  await open(page, "#/people");
  await expect(page.getByRole("heading", { level: 2, name: "Ansiktsindeksen er av" })).toBeVisible();
  await expect(page.getByText("Analysen skjer bare på denne PC-en")).toBeVisible();
  await expect(page.getByText("originalbildene endres aldri")).toBeVisible();
  await expect(page.getByRole("list", { name: "Personer" })).toHaveCount(0);
});

test("samtykke: dialogen forklarer, Esc og Avbryt lar indeksen stå av, Slå på viser personene", async ({ page }) => {
  await open(page, "#/people");
  const btn = page.getByRole("button", { name: "Slå på ansiktsindeks" });
  await btn.click();
  const dlg = page.getByRole("dialog", { name: "Slå på ansiktsindeks?" });
  await expect(dlg).toBeVisible();
  for (const t of ["Hva som lagres", "Det forlater aldri enheten", "Originalene endres aldri", "Du kan slå av og slette"]) await expect(dlg.getByText(t)).toBeVisible();
  await expect(dlg.getByRole("button", { name: "Avbryt" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(btn).toBeFocused();
  await btn.click();
  await dlg.getByRole("button", { name: "Avbryt" }).click();
  await expect(page.getByRole("heading", { level: 2, name: "Ansiktsindeksen er av" })).toBeVisible();
  await btn.click();
  await dlg.getByRole("button", { name: "Slå på", exact: true }).click();
  await expect(page.getByRole("list", { name: "Personer" }).getByRole("listitem")).toHaveCount(4);
  await expect(page.getByText("Ansiktsindeksen er slått på.")).toBeAttached();
});

test("navn: knapp og tastatur, Esc avbryter, Angre gir forrige navn", async ({ page }) => {
  await open(page, "#/people");
  await enable(page);
  const add = page.getByRole("button", { name: "Gi navn til Uten navn, 3 bilder" });
  await add.focus();
  await page.keyboard.press("Enter"); // samme handling som klikk
  const field = page.getByRole("textbox", { name: "Navn" });
  await expect(field).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(field).toHaveCount(0);
  await expect(add).toBeFocused();
  await add.click();
  await field.fill("Eva");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 2, name: "Eva" })).toBeVisible();
  const toast = page.getByRole("status").filter({ hasText: "Navnet på Eva er endret." });
  await expect(toast).toBeVisible();
  await toast.getByRole("button", { name: "Angre" }).click();
  await expect(page.getByRole("heading", { level: 2, name: "Eva" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Gi navn til Uten navn, 3 bilder" })).toBeVisible();
});

test("navn: for langt gir feil ved feltet", async ({ page }) => {
  await open(page, "#/people");
  await enable(page);
  await page.getByRole("button", { name: "Gi navn til Uten navn, 3 bilder" }).click();
  await page.getByRole("textbox", { name: "Navn" }).fill("x".repeat(101));
  await page.getByRole("button", { name: "Lagre navn" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "Navnet er for langt." })).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Navn" })).toHaveAttribute("aria-invalid", "true");
});

test("sammenslåing: tydelig valg, oppsummering, Angre deler personene igjen", async ({ page }) => {
  await open(page, "#/people");
  await enable(page);
  await page.getByRole("button", { name: "Slå sammen Uten navn, 3 bilder med en annen person" }).click();
  const dlg = page.getByRole("dialog", { name: "Slå sammen personer" });
  await expect(dlg).toBeVisible();
  await expect(dlg.getByRole("button", { name: "Slå sammen" })).toBeDisabled();
  await dlg.getByRole("radio", { name: "Kari, 6 bilder" }).check();
  await expect(dlg.getByText("Uten navn slås inn i Kari.")).toBeVisible();
  await expect(dlg.getByRole("radio", { name: "Kari, 6 bilder" }).nth(1)).toBeChecked(); // «Hvem skal bli stående»: personen med navn
  await dlg.getByRole("button", { name: "Slå sammen" }).click();
  await expect(dlg).toBeHidden();
  await expect(page.getByRole("list", { name: "Personer" }).getByRole("listitem")).toHaveCount(3);
  const toast = page.getByRole("status").filter({ hasText: "Uten navn er slått sammen med Kari." });
  await expect(toast).toBeVisible();
  await toast.getByRole("button", { name: "Angre" }).click();
  await expect(page.getByRole("list", { name: "Personer" }).getByRole("listitem")).toHaveCount(4);
  // Avbryt rører ingenting
  await page.getByRole("button", { name: "Slå sammen Jonas, 5 bilder med en annen person" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Avbryt" }).click();
  await expect(page.getByRole("list", { name: "Personer" }).getByRole("listitem")).toHaveCount(4);
});

test("beskyttelse: bryter med aria-pressed og Angre", async ({ page }) => {
  await open(page, "#/people");
  await enable(page);
  const t = page.getByRole("button", { name: "Beskytt Kari, 6 bilder" });
  await expect(t).toHaveAttribute("aria-pressed", "false");
  await t.click();
  await expect(t).toHaveAttribute("aria-pressed", "true");
  const toast = page.getByRole("status").filter({ hasText: "Kari er beskyttet." });
  await expect(toast).toBeVisible();
  await toast.getByRole("button", { name: "Angre" }).click();
  await expect(t).toHaveAttribute("aria-pressed", "false");
});

test("innstillinger: på krever samtykke, slett ansiktsindeks bekreftes og sier ærlig at det ikke kan angres", async ({ page }) => {
  await open(page, "#/settings");
  const card = page.getByRole("group", { name: "Ansikter og personer" });
  await expect(card.getByRole("radio", { name: "Av" })).toBeChecked();
  await card.getByRole("radio", { name: "På" }).check();
  const consent = page.getByRole("dialog", { name: "Slå på ansiktsindeks?" });
  await expect(consent).toBeVisible();
  await consent.getByRole("button", { name: "Avbryt" }).click();
  await expect(card.getByRole("radio", { name: "Av" })).toBeChecked();
  await card.getByRole("radio", { name: "På" }).check();
  await consent.getByRole("button", { name: "Slå på", exact: true }).click();
  await expect(card.getByRole("radio", { name: "På" })).toBeChecked();

  await go(page, "#/people");
  await expect(page.getByRole("heading", { level: 2, name: "Kari" })).toBeVisible();
  await go(page, "#/settings");
  await card.getByRole("button", { name: "Slett ansiktsindeksen" }).click();
  const dlg = page.getByRole("dialog", { name: "Slette ansiktsindeksen?" });
  await expect(dlg.getByText("Dette kan ikke angres.")).toBeVisible();
  await expect(dlg.getByText("beskyttelse av personer faller bort")).toBeVisible();
  await expect(dlg.getByRole("button", { name: "Avbryt" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await go(page, "#/people");
  await expect(page.getByRole("heading", { level: 2, name: "Kari" })).toBeVisible(); // avbrutt: ingenting slettet
  await go(page, "#/settings");
  await card.getByRole("button", { name: "Slett ansiktsindeksen" }).click();
  await dlg.getByRole("button", { name: "Slett ansiktsindeksen" }).click();
  await expect(dlg).toBeHidden();
  await expect(card.getByText("33 ansikter er slettet fra indeksen.")).toBeVisible();
  await go(page, "#/people");
  await expect(page.getByRole("heading", { level: 2, name: "Ingen personer funnet ennå" })).toBeVisible();
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of Object.entries({ bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } })) {
    test(`axe: personer og ansiktsindeks ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(120_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await open(page, "#/people");
      await axe(page, "personer, indeks av");
      await page.getByRole("button", { name: "Slå på ansiktsindeks" }).click();
      await axe(page, "samtykkedialog");
      await page.getByRole("dialog").getByRole("button", { name: "Slå på", exact: true }).click();
      await expect(page.getByRole("heading", { level: 2, name: "Kari" })).toBeVisible();
      await axe(page, "personer, liste");
      await page.getByRole("button", { name: "Gi navn til Uten navn, 3 bilder" }).click();
      await page.getByRole("textbox", { name: "Navn" }).fill("x".repeat(101));
      await page.getByRole("button", { name: "Lagre navn" }).click();
      await axe(page, "navnefelt med feil");
      await page.getByRole("button", { name: "Avbryt" }).click();
      await page.getByRole("button", { name: "Beskytt Kari, 6 bilder" }).click();
      await expect(page.getByRole("button", { name: "Angre" })).toBeVisible();
      await axe(page, "personer med Toast");
      await page.getByRole("button", { name: "Slå sammen Uten navn, 3 bilder med en annen person" }).click();
      await page.getByRole("dialog").getByRole("radio", { name: "Kari, 6 bilder" }).check();
      await axe(page, "sammenslåingsdialog");
      await page.keyboard.press("Escape");
      await go(page, "#/settings");
      await axe(page, "innstillinger");
      await page.getByRole("button", { name: "Slett ansiktsindeksen" }).click();
      await axe(page, "slettedialog");
    });
  }
}

for (const lang of ["nb", "en", "pseudo"] as Lang[]) {
  for (const [vp, size] of Object.entries({ bred: { width: 1280, height: 800 }, smal: { width: 360, height: 740 } })) {
    test(`layout uten avkutting: ${lang} ${vp}`, async ({ page }) => {
      test.setTimeout(90_000);
      await page.setViewportSize(size);
      await open(page, "#/people", lang);
      const check = async (where: string): Promise<void> => {
        await page.waitForTimeout(100);
        const problems = await page.evaluate(() => {
          const out: string[] = [];
          if (document.documentElement.scrollWidth > innerWidth + 1) out.push("horisontal sideskroll");
          for (const el of Array.from(document.querySelectorAll("button, h1, h2, p, label"))) {
            if (el.closest(".sr-only") || el.closest("[aria-hidden='true']")) continue;
            const r = el.getBoundingClientRect();
            if (r.width === 0) continue;
            if (r.right > innerWidth + 1) out.push(`${el.tagName} utenfor vinduet: ${el.textContent?.slice(0, 30)}`);
            if (el.tagName === "BUTTON" && el.scrollWidth > el.clientWidth + 1) out.push(`knapp renner over: ${el.textContent?.slice(0, 30)}`);
          }
          return out;
        });
        expect(problems, where).toEqual([]);
      };
      await check("tom tilstand");
      await page.locator("main button.primary, section button.primary").first().click();
      await check("samtykkedialog");
      await page.getByRole("dialog").locator("button.primary").click();
      await page.locator("li.person").first().waitFor();
      await check("liste");
      await page.locator("li.person").nth(1).locator("button").nth(1).click(); // Slå sammen
      await check("sammenslåing");
    });
  }
}
