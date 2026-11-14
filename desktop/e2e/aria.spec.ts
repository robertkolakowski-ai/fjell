// UX-6 ARIA-gjennomgang for Narrator/NVDA: landemerker, overskrifter, fokus ved skjermbytte, dialoger (fokusfelle,
// navn, retur av fokus) og live-regioner. Automatisk dekning av det som kan testes uten skjermleser;
// resten står i docs/BETA.md (manuelt skript).
import { expect, test, type Page } from "@playwright/test";
import { tour } from "./tour";
import { go, open, seedScan } from "./helpers";

/** Fokus er i dialogen, eller har forlatt siden (Tab forbi siste element går til nettleser-/vinduskrom i Chromium/WebView2).
 *  Det som aldri må skje: fokus i bakgrunnen (header, main) mens modal dialog er åpen. */
const activeInside = (page: Page, sel: string) =>
  page.evaluate((s) => {
    const a = document.activeElement;
    if (!a || a === document.body || a === document.documentElement) return true;
    return !!a.closest(s);
  }, sel);

test("landemerker og overskrifter på alle skjermer", async ({ page }) => {
  test.setTimeout(120_000);
  const problems: string[] = [];
  await tour(page, async (name) => {
    if (name.includes("hjelp") || name.includes("ark")) return; // dialog åpen: resten av siden er inert
    const r = await page.evaluate(() => {
      const out: string[] = [];
      const count = (sel: string) => document.querySelectorAll(sel).length;
      // <header> er banner bare utenfor main/section/article/aside/nav.
      const banners = Array.from(document.querySelectorAll("header, [role=banner]")).filter((h) => h.getAttribute("role") === "banner" || !h.closest("main, section, article, aside, nav"));
      if (banners.length !== 1) out.push(`banner: ${banners.length} (forventet 1)`);
      if (count("main, [role=main]") !== 1) out.push(`main: ${count("main, [role=main]")} (forventet 1)`);
      const navs = Array.from(document.querySelectorAll("nav, [role=navigation]"));
      if (navs.length < 1 || navs.some((n) => !(n.getAttribute("aria-label") || n.getAttribute("aria-labelledby")))) out.push("nav uten navn");
      // Region-landemerker (section med navn, role=region) må ha unikt navn; for mange gir støy i landemerkelisten.
      const regions = Array.from(document.querySelectorAll("section[aria-label], section[aria-labelledby], [role=region]"));
      const names = regions.map((x) => x.getAttribute("aria-label") ?? document.getElementById(x.getAttribute("aria-labelledby") ?? "")?.textContent ?? "");
      if (names.some((n) => !n?.trim())) out.push("region uten navn");
      if (new Set(names).size !== names.length) out.push(`region med duplisert navn: ${names.join(" | ")}`);
      if (regions.length > 4) out.push(`${regions.length} region-landemerker (maks 4 for å unngå støy for Narrator/NVDA)`);
      if (count("h1") !== 1) out.push(`h1: ${count("h1")} (forventet 1)`);
      const levels = Array.from(document.querySelectorAll("h1,h2,h3,h4,h5,h6")).map((h) => Number(h.tagName[1]));
      for (let i = 1; i < levels.length; i++) if (levels[i] - levels[i - 1] > 1) out.push(`overskriftsnivå hopper ${levels[i - 1]} -> ${levels[i]}`);
      // Duplisert navn: aria-label som gjentar overskriften rett inni gir dobbel opplesning.
      for (const s of Array.from(document.querySelectorAll("section[aria-label]"))) {
        const h = s.querySelector("h2,h3,h4");
        if (h && h.textContent?.trim() === s.getAttribute("aria-label")) out.push(`section aria-label gjentar overskrift: «${h.textContent?.trim()}»`);
      }
      if (document.documentElement.lang === "") out.push("html lang mangler");
      return out;
    });
    problems.push(...r.map((x) => `${name}: ${x}`));
  });
  expect(problems, problems.join("\n")).toEqual([]);
});

test("fokus flyttes til skjermtittel ved skjermbytte, og hopp-lenke virker", async ({ page }) => {
  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/");
  await page.getByRole("button", { name: "Rask opprydding" }).waitFor();
  for (const link of ["Gjennomgang", "Papirkurv", "Innstillinger", "Hjem"]) {
    await page.getByRole("navigation").getByRole("link", { name: new RegExp(`^${link}`) }).click();
    await expect.poll(() => page.evaluate(() => document.activeElement?.tagName)).toBe("H1");
    expect(await page.evaluate(() => !!document.activeElement?.closest("main"))).toBe(true);
  }
  // Hopp-lenke er første tabstopp og flytter fokus til main.
  await page.goto("about:blank");
  await open(page, "#/settings"); // ny sidelasting: Tab starter fra toppen av dokumentet
  await page.keyboard.press("Tab");
  expect(await page.evaluate(() => document.activeElement?.className.includes("skip") || document.activeElement?.getAttribute("href") === "#main")).toBe(true);
  await page.keyboard.press("Enter");
  expect(await page.evaluate(() => document.activeElement?.id)).toBe("main");
  // Dokumenttittel følger skjermen (Narrator leser tittelen ved vindusfokus).
  await go(page, "#/settings");
  await expect(page).toHaveTitle(/Innstillinger/);
});

test("dialog: navn, fokusfelle (Tab og Shift+Tab), Esc lukker og fokus går tilbake til utløser", async ({ page }) => {
  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/");
  await page.getByRole("button", { name: "Rask opprydding" }).waitFor();
  const opener = page.getByRole("button", { name: "Hurtigtaster" });
  await opener.click();
  const dlg = page.getByRole("dialog");
  await expect(dlg).toBeVisible();
  await expect(dlg).toHaveAccessibleName(/\S/);
  expect(await page.evaluate(() => !!document.activeElement?.closest("dialog"))).toBe(true); // fokus flyttes inn ved åpning
  for (let i = 0; i < 12; i++) { await page.keyboard.press("Tab"); expect(await activeInside(page, "dialog"), `Tab ${i}`).toBe(true); }
  for (let i = 0; i < 12; i++) { await page.keyboard.press("Shift+Tab"); expect(await activeInside(page, "dialog"), `Shift+Tab ${i}`).toBe(true); }
  // Bakgrunnen er utilgjengelig for hjelpemidler mens modal dialog er åpen.
  expect(await page.evaluate(() => document.querySelector("dialog")?.matches(":modal"))).toBe(true);
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect.poll(() => page.evaluate(() => document.activeElement?.textContent?.trim())).toBe("Hurtigtaster");
});

test("ark (Tøm papirkurv): navn, fokusfelle og retur av fokus", async ({ page }) => {
  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/review/blurry");
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt 6/ }).click();
  await go(page, "#/trash");
  const opener = page.getByRole("button", { name: "Tøm papirkurv …" });
  await opener.click();
  const dlg = page.getByRole("dialog");
  await expect(dlg).toBeVisible();
  await expect(dlg).toHaveAccessibleName(/\S/);
  expect(await page.evaluate(() => !!document.activeElement?.closest("dialog"))).toBe(true);
  for (let i = 0; i < 14; i++) { await page.keyboard.press("Tab"); expect(await activeInside(page, "dialog"), `Tab ${i}`).toBe(true); }
  await page.keyboard.press("Shift+Tab");
  expect(await activeInside(page, "dialog")).toBe(true);
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect.poll(() => page.evaluate(() => document.activeElement?.textContent?.trim())).toContain("Tøm papirkurv");
});

// Tidligere funn (UX-6) rettet i M9: Review.svelte satte inn role=status-elementer sammen med teksten («Flytter …», «6 flyttet»).
// Narrator/NVDA annonserer slike ofte ikke. Nå står en vedvarende aria-live-region i bunnlinjen og får teksten inn i seg.
test("live-regioner i skjermene: finnes før innholdet endres (Narrator/NVDA)", async ({ page }) => {
  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/review/blurry");
  await page.locator(".tile").first().waitFor();
  // Bunnlinjens live-region skal være i DOM (tom eller med tekst) før vi velger noe.
  await page.locator(".tile").first().click();
  await expect(page.locator(".bar [role=status]")).toHaveCount(1);
  // Observer: ingen role=status/alert-element settes inn med ferdig tekst, utenom de som allerede finnes.
  await page.evaluate(() => {
    (window as unknown as { __late: string[] }).__late = [];
    new MutationObserver((muts) => {
      for (const m of muts) for (const n of Array.from(m.addedNodes)) {
        if (!(n instanceof HTMLElement)) continue;
        const live = n.matches("[role=status],[aria-live]") ? [n] : Array.from(n.querySelectorAll("[role=status],[aria-live]"));
        for (const l of live) if ((l.textContent ?? "").trim() && l.getAttribute("role") !== "alert") (window as unknown as { __late: string[] }).__late.push((l.textContent ?? "").trim().slice(0, 40));
      }
    }).observe(document.body, { childList: true, subtree: true });
  });
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt 6/ }).click();
  await expect(page.getByText("6 flyttet")).toBeVisible();
  await page.waitForTimeout(500);
  const late = await page.evaluate(() => (window as unknown as { __late: string[] }).__late);
  expect(late, `live-regioner satt inn med ferdig tekst: ${late.join(" | ")}`).toEqual([]);
});

test("alle ikon-SVG-er er skjult for skjermleser, og knapper har tilgjengelig navn", async ({ page }) => {
  test.setTimeout(120_000);
  const problems: string[] = [];
  await tour(page, async (name) => {
    const r = await page.evaluate(() => {
      const out: string[] = [];
      for (const s of Array.from(document.querySelectorAll("svg"))) {
        const hidden = s.closest("[aria-hidden=true]") || s.getAttribute("aria-hidden") === "true" || s.getAttribute("role") === "img";
        if (!hidden) out.push("svg uten aria-hidden");
      }
      for (const b of Array.from(document.querySelectorAll("button"))) {
        const name = (b.getAttribute("aria-label") ?? b.textContent ?? "").trim();
        if (!name && !b.getAttribute("aria-labelledby")) out.push("knapp uten navn");
      }
      for (const i of Array.from(document.querySelectorAll("img"))) if (!i.hasAttribute("alt")) out.push("img uten alt");
      return out;
    });
    problems.push(...r.map((x) => `${name}: ${x}`));
  });
  expect(problems, problems.join("\n")).toEqual([]);
});

test("komponentene: BottomBar har vedvarende live-region", async ({ page }) => {
  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/review/blurry");
  await page.locator(".tile").first().waitFor();
  expect(await page.locator(".bar [role=status]").count()).toBe(1); // finnes før valg
  await page.locator(".tile").first().click();
  await expect(page.locator(".bar [role=status]")).toHaveCount(1);
  await expect(page.locator(".bar [role=status]")).toContainText(/\d/); // tekst kommer inn i samme region
});

test("komponentene: Toast-regionen finnes før teksten (Gallery, dev)", async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as unknown as { __toast: { textAtInsert: string; later: string } };
    w.__toast = { textAtInsert: "?", later: "" };
    new MutationObserver((muts) => {
      for (const m of muts) for (const n of Array.from(m.addedNodes)) {
        if (n instanceof HTMLElement && n.matches(".toast")) { w.__toast.textAtInsert = (n.querySelector(".msg")?.textContent ?? "").replace(/\u00a0/g, "").trim(); }
      }
    }).observe(document, { childList: true, subtree: true });
  });
  await page.goto("/?gallery");
  await page.locator(".toast").waitFor();
  const t0 = await page.evaluate(() => (window as unknown as { __toast: { textAtInsert: string } }).__toast.textAtInsert);
  expect(t0).toBe(""); // regionen ble satt inn uten tekst
  await expect(page.locator(".toast .msg")).toContainText(/\d/, { timeout: 2000 }); // og får tekst like etter
  await page.setViewportSize({ width: 360, height: 700 });
  await expect(page.locator(".toast .undo")).toBeVisible();
});
