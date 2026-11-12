// Beslutning M9: filer Papirkurv ikke kan ta (`Unsupported`: volum uten Papirkurv, lang sti uten 8.3-navn, UNC) ligger urørt
// i karantenen. Arket tilbyr «La ligge i karantenen» (standard) og «Slett permanent …» (eget trinn med avkrysning).
// MockBridge styres med `?mockUnsupported=N` (bare dev). Dekker mus, tastatur, nb/en/pseudo og axe (lys/mørk × bred/smal).
import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { expect, test, type Page } from "@playwright/test";
import type { Lang } from "./helpers";

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

const T = {
  nb: {
    start: /Kom i gang/, scan: /Start skanning/, move: /^Flytt 6/, all: "Velg alle", empty: "Tøm papirkurv …", cont: "Fortsett",
    title: "Noen bilder kan ikke flyttes til Papirkurv", keep: "La ligge i karantenen", del: "Slett permanent …",
    permTitle: "Dette kan ikke angres", cancel: "Avbryt", go: "Slett 2 permanent", kept: "2 bilder ligger urørt i karantenen. Ingenting er slettet.",
    moved: "Flyttet til Windows Papirkurv", body: /2 bilder ligger på et sted uten Windows Papirkurv/,
  },
  en: {
    start: /Get started/, scan: /Start scan/, move: /^Move 6/, all: "Select all", empty: "Empty trash …", cont: "Continue",
    title: "Some photos cannot be moved to the Recycle Bin", keep: "Keep in quarantine", del: "Delete permanently …",
    permTitle: "This can't be undone", cancel: "Cancel", go: "Delete 2 permanently", kept: "2 photos are untouched in quarantine. Nothing has been deleted.",
    moved: "Moved to the Windows Recycle Bin", body: /2 photos are on a location without a Windows Recycle Bin/,
  },
} as const;

/** Åpner appen med 2 Unsupported-filer, flytter 6 til «Nylig slettet» og åpner Tøm-arket. */
async function toUnsupported(page: Page, lang: Lang, unsupported: number | "all" = 2): Promise<void> {
  const t = lang === "en" ? T.en : T.nb;
  await page.addInitScript((l) => { try { localStorage.setItem("fjell.language", l); } catch { /* ignorer */ } }, lang);
  await page.goto(`/?mockDelay=0&mockUnsupported=${unsupported}#/onboarding`);
  if (lang === "pseudo") {
    await page.locator("h1").first().waitFor();
    await page.locator(".onb > .row button").first().click();
    await page.waitForTimeout(250);
    await page.locator(".onb > .row button").last().click(); // «Start skanning» er siste knapp
  } else {
    await page.getByRole("button", { name: t.start }).click();
    await page.getByRole("button", { name: t.scan }).click();
  }
  await page.locator("h1").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 }); // onboarding ferdig (uavhengig av språk)
  await page.evaluate(() => { location.hash = "#/review/blurry"; });
  await page.locator(".tile").first().waitFor();
  if (lang === "pseudo") {
    await page.locator(".tile").nth(0).click(); // to valgte holder for pseudo-sjekken
    await page.locator(".tile").nth(1).click();
    await page.locator(".bar button").last().click();
  } else {
    await page.getByRole("button", { name: t.all }).click();
    await page.getByRole("button", { name: t.move }).click();
  }
  await page.locator(".bar [role=status]").first().waitFor();
  await page.evaluate(() => { location.hash = "#/trash"; });
  await page.locator(".trash .summary").waitFor();
}

async function openSheet(page: Page, lang: Lang): Promise<void> {
  const t = lang === "en" ? T.en : T.nb;
  await page.getByRole("button", { name: t.empty }).click();
  await page.getByRole("button", { name: t.cont }).click();
}

for (const lang of ["nb", "en"] as const) {
  const t = T[lang];
  test(`Unsupported (${lang}): La ligge er standard, med mus`, async ({ page }) => {
    await toUnsupported(page, lang);
    await openSheet(page, lang);
    const dlg = page.getByRole("dialog", { name: t.title });
    await expect(dlg).toBeVisible();
    await expect(dlg).toContainText(t.body);
    await expect(dlg.getByRole("button", { name: t.keep })).toBeFocused(); // standardvalg har fokus
    await dlg.getByRole("button", { name: t.keep }).click();
    await expect(dlg).toBeHidden();
    const done = page.locator(".done");
    await expect(done).toContainText(t.moved); // 4 av 6 ble flyttet
    await expect(done).toContainText(t.kept);
    await expect(page.getByRole("status").filter({ hasText: t.kept }).first()).toBeAttached();
    await expect(page.locator(".items li")).toHaveCount(2); // de to ligger fortsatt i «Nylig slettet», urørt
    await expect(page.locator(".history")).toHaveCount(0); // ingenting «frigjort»
  });

  test(`Unsupported (${lang}): permanent bare med avkrysning, Avbryt tilbake til valgene`, async ({ page }) => {
    await toUnsupported(page, lang);
    await openSheet(page, lang);
    await page.getByRole("dialog", { name: t.title }).getByRole("button", { name: t.del }).click();
    const perm = page.getByRole("dialog", { name: t.permTitle });
    await expect(perm).toBeVisible();
    await expect(perm.getByRole("button", { name: t.go })).toBeDisabled();
    await perm.getByRole("button", { name: t.cancel }).click();
    await expect(page.getByRole("dialog", { name: t.title })).toBeVisible(); // tilbake, ingenting slettet
    await expect(page.locator(".items li")).toHaveCount(2);
    await page.getByRole("button", { name: t.del }).click();
    await perm.getByRole("checkbox").check();
    await perm.getByRole("button", { name: t.go }).click();
    await expect(page.locator(".done")).toContainText(/permanent|permanently/);
    await expect(page.locator(".items li")).toHaveCount(0);
  });
}

test("Unsupported: bare tastatur (Enter = la ligge, Escape = la ligge, permanent med Tab og mellomrom)", async ({ page }) => {
  await toUnsupported(page, "nb");
  await openSheet(page, "nb");
  const dlg = page.getByRole("dialog", { name: T.nb.title });
  await expect(dlg).toBeVisible();
  await page.keyboard.press("Enter"); // fokus ligger på «La ligge»
  await expect(page.locator(".done")).toContainText(T.nb.kept);
  await expect(page.locator(".items li")).toHaveCount(2);

  // Escape
  await page.getByRole("button", { name: "Ferdig" }).click();
  await openSheet(page, "nb");
  await expect(dlg).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dlg).toBeHidden();
  await expect(page.locator(".done")).toContainText(T.nb.kept);
  await expect(page.locator(".items li")).toHaveCount(2);

  // Permanent: Tab fra «La ligge» til «Slett permanent …», Enter, mellomrom på avkrysningen, Tab til knapp
  await page.getByRole("button", { name: "Ferdig" }).click();
  await openSheet(page, "nb");
  await page.keyboard.press("Tab");
  await expect(dlg.getByRole("button", { name: T.nb.del })).toBeFocused();
  await page.keyboard.press("Enter");
  const perm = page.getByRole("dialog", { name: T.nb.permTitle });
  await expect(perm).toBeVisible();
  await expect(perm.getByRole("checkbox")).toBeFocused(); // fokus flyttes til avkrysningen ved trinnskiftet
  await page.keyboard.press("Space");
  await page.keyboard.press("Tab"); // Avbryt
  await page.keyboard.press("Tab"); // Slett 2 permanent (destruktiv, sist)
  await expect(perm.getByRole("button", { name: T.nb.go })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator(".done")).toContainText("permanent slettet");
  await expect(page.locator(".items li")).toHaveCount(0);
});

test("Unsupported: volum uten Papirkurv (alle filer) gir samme ark og ingenting flyttes eller slettes", async ({ page }) => {
  await toUnsupported(page, "nb", "all");
  await openSheet(page, "nb");
  await expect(page.getByRole("dialog", { name: T.nb.title })).toContainText("6 bilder ligger på et sted uten Windows Papirkurv");
  await page.keyboard.press("Enter");
  await expect(page.locator(".done")).toContainText("6 bilder ligger urørt i karantenen");
  await expect(page.locator(".done")).not.toContainText(T.nb.moved);
  await expect(page.locator(".items li")).toHaveCount(6);
});

test("Unsupported: pseudo-lokale uten avkutting i arket (smal)", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 740 });
  await toUnsupported(page, "pseudo");
  await page.locator(".actions button:visible").last().click();
  await page.locator("dialog button:visible").last().click();
  const dlg = page.locator("dialog[open]");
  await expect(dlg).toBeVisible();
  const clipped = await dlg.evaluate((d) => {
    const bad: string[] = [];
    for (const el of d.querySelectorAll<HTMLElement>("h2, p, button")) {
      if (el.scrollWidth > el.clientWidth + 1) bad.push(el.tagName + ":" + el.textContent);
    }
    return { bad, overflowX: d.scrollWidth > d.clientWidth + 1 };
  });
  expect(clipped).toEqual({ bad: [], overflowX: false });
});

for (const scheme of ["light", "dark"] as const) {
  for (const [vp, size] of [["desktop", { width: 1280, height: 800 }], ["smal", { width: 360, height: 740 }]] as const) {
    test(`axe Unsupported ${scheme} ${vp}`, async ({ page }) => {
      test.setTimeout(90_000);
      await page.setViewportSize(size);
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
      await toUnsupported(page, "nb");
      await openSheet(page, "nb");
      await expect(page.getByRole("dialog", { name: T.nb.title })).toBeVisible();
      await axe(page, "Unsupported-ark");
      await page.getByRole("button", { name: T.nb.del }).click();
      await expect(page.getByRole("dialog", { name: T.nb.permTitle })).toBeVisible();
      await axe(page, "permanent-trinn fra Unsupported");
      await page.getByRole("button", { name: T.nb.cancel }).click();
      await page.getByRole("button", { name: T.nb.keep }).click();
      await expect(page.locator(".done")).toContainText(T.nb.kept);
      await axe(page, "resultat etter La ligge");
    });
  }
}

// Skjermbilder til docs/screens/m9/ (bare med SHOTS=1; JPEG, bare skjermene denne fasen endrer).
const OUT = process.env.SHOTS_DIR ?? "../docs/screens/m9";
async function shot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${OUT}/${name}.jpg`, type: "jpeg", quality: 55 });
}
for (const [lang, scheme] of [["nb", "light"], ["nb", "dark"], ["en", "light"]] as const) {
  test(`skjermbilder Unsupported ${lang} ${scheme}`, async ({ page }) => {
    test.skip(!process.env.SHOTS, "bare med SHOTS=1");
    test.setTimeout(90_000);
    mkdirSync(OUT, { recursive: true });
    const t = T[lang];
    await page.setViewportSize({ width: 900, height: 620 });
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
    await toUnsupported(page, lang);
    await openSheet(page, lang);
    await shot(page, `papirkurv-unsupported-ark-${lang}-${scheme}`);
    if (lang === "nb" && scheme === "light") {
      await page.getByRole("button", { name: t.del }).click();
      await shot(page, `papirkurv-unsupported-permanent-${lang}-${scheme}`);
      await page.getByRole("button", { name: t.cancel }).click();
    }
    await page.getByRole("button", { name: t.keep }).click();
    await expect(page.locator(".done")).toContainText(t.kept);
    if (lang === "nb") await shot(page, `papirkurv-unsupported-resultat-${lang}-${scheme}`);
  });
}
test("skjermbilde Unsupported pseudo (smal)", async ({ page }) => {
  test.skip(!process.env.SHOTS, "bare med SHOTS=1");
  mkdirSync(OUT, { recursive: true });
  await page.setViewportSize({ width: 420, height: 760 });
  await toUnsupported(page, "pseudo");
  await page.locator(".actions button:visible").last().click();
  await page.locator("dialog button:visible").last().click();
  await expect(page.locator("dialog[open]")).toBeVisible();
  await shot(page, "papirkurv-unsupported-ark-pseudo-smal");
});
