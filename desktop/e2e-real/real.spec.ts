// Ekte-app-e2e: Playwright mot fjell-desktop.exe (Tauri + WebView2) over CDP. Kun Windows. Se docs/mvp/E2E-WINDOWS.md.
// Scenariene er seriekjørt og deler én tilstandskjede (førstegang -> flytt/angre -> omstart -> Papirkurv); skymappe har egen APPDATA.
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { generate, type Manifest } from "../scripts/real/fixtures.mjs";
import { DATA_IDENTIFIER, homeDirs, launchApp, type App } from "./launch";
import { purgeTestFilesFromRecycleBin, recycleBinNames, recycleGuardOk } from "./ps";
import { S } from "./strings";

const OUT = process.env.FJELL_REAL_OUT ?? join(fileURLToPath(new URL(".", import.meta.url)), ".results");
mkdirSync(OUT, { recursive: true });

// ---------- ytelsesmåling (kun rapport, ingen porter ennå) ----------
const perf: { name: string; ms: number }[] = [];
const cold: Record<string, number> = {};
const pct = (xs: number[], p: number) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[Math.min(s.length - 1, Math.ceil(p * s.length) - 1)]! : null; };

/** Klikker `loc` og måler (i siden) til `done(before)` er sann i DOM + neste frame. */
async function timedClick(page: Page, name: string, loc: Locator, done: string): Promise<number> {
  const ms = await loc.evaluate((el, doneSrc) => new Promise<number>((resolve, reject) => {
    const before = document.querySelector("h1")?.textContent ?? "";
    const ok = new Function("before", `return (${doneSrc});`) as (b: string) => boolean;
    const fail = setTimeout(() => { mo.disconnect(); reject(new Error("tidsavbrudd (10 s) ventet på: " + doneSrc)); }, 10_000);
    const finish = (t0: number) => { clearTimeout(fail); mo.disconnect(); requestAnimationFrame(() => resolve(performance.now() - t0)); };
    let t0 = 0;
    const mo = new MutationObserver(() => { if (ok(before)) finish(t0); });
    mo.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
    t0 = performance.now();
    (el as HTMLElement).click();
  }), done);
  perf.push({ name, ms });
  return ms;
}

// ---------- felles tilstand ----------
let root = "";
let manifest: Manifest;
let app: App | undefined;
let homeA = homeDirs("");
const lib = () => manifest.photosDir;
const abs = (rel: string) => join(lib(), rel);
const sha = (p: string) => createHash("sha256").update(readFileSync(p)).digest("hex");
const missingFiles = () => manifest.files.filter((f) => !existsSync(abs(f.rel)));
// Tauri bruker SHGetKnownFolderPath (ikke APPDATA-env), så ekte datamappe kan være den ekte %APPDATA%. Vi sjekker begge.
const realAppData = process.env.APPDATA ?? "";
const dataDirs = (h: typeof homeA) => [join(h.appdata, DATA_IDENTIFIER), ...(realAppData ? [join(realAppData, DATA_IDENTIFIER)] : [])];
const dataDirExists = (h: typeof homeA) => dataDirs(h).some((d) => existsSync(d));
let armed = false;
/** Fjerner datamappen før en «ren» start. Lokalt (ikke CI) nekter vi å slette en eksisterende ekte datamappe. */
function freshDataDir(h: typeof homeA): void {
  for (const d of dataDirs(h)) {
    if (!existsSync(d)) continue;
    if (process.env.CI !== "true" && !d.startsWith(h.appdata)) throw new Error(`Ekte datamappe finnes allerede (${d}). Flytt/slett den selv før lokal kjøring; testen sletter den ikke utenfor CI.`);
    rmSync(d, { recursive: true, force: true });
  }
}

async function hash(page: Page, h: string): Promise<void> {
  await page.evaluate((x) => { location.hash = x; }, h);
  await page.locator("h1").first().waitFor();
}

/** Onboarding: Kom i gang, fjern standardmapper, skriv stien, Start skanning. */
async function onboardAndScan(page: Page, folder: string): Promise<void> {
  await page.getByRole("button", { name: S.welcomeStart }).click();
  await expect(page.getByRole("heading", { level: 1, name: S.foldersTitle })).toBeVisible();
  for (let i = 0; i < 5; i++) {
    const rm = page.getByRole("button", { name: S.folderRemove });
    if (!(await rm.count())) break;
    await rm.first().click();
  }
  await page.getByRole("button", { name: S.folderAdd }).click();
  const dlg = page.getByRole("dialog");
  await dlg.getByRole("textbox", { name: S.folderPath }).fill(folder);
  await dlg.getByRole("button", { name: S.folderSubmit }).click();
  await expect(dlg).toBeHidden({ timeout: 15_000 });
  await expect(page.getByText(folder, { exact: false }).first(), "valgt mappe skal vises i listen").toBeVisible();
  await page.getByRole("button", { name: S.scanStart }).click();
}

/** Venter til overskriften «Fant N eksakte duplikater» er stabil i 2 s (skann ferdig for eksakte duplikater). Returnerer N. */
async function scanSettled(page: Page): Promise<number> {
  const h1 = page.getByRole("heading", { level: 1 });
  await expect(h1).toContainText(S.foundExact, { timeout: 120_000 });
  let last = "", since = Date.now();
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    const t = (await h1.first().textContent()) ?? "";
    if (t !== last) { last = t; since = Date.now(); }
    else if (Date.now() - since > 2000) break;
    await page.waitForTimeout(200);
  }
  const m = S.foundExact.exec(last);
  if (!m) throw new Error(`Skann ble ikke ferdig med duplikatoverskrift; siste overskrift: «${last}»`);
  return Number(m[1]);
}

/** Åpner første gruppe fra Gjennomgang, flytter den (Behold dette) og returnerer. */
async function commitFirstGroup(page: Page, label: string): Promise<void> {
  await hash(page, "#/review/exact_dup");
  const link = page.getByRole("link", { name: S.groupLink }).first();
  await expect(link, "ingen gruppe i Gjennomgang").toBeVisible({ timeout: 30_000 });
  await timedClick(page, "åpne gruppe", link, "location.hash.startsWith('#/group/') && !!document.querySelector('h1')");
  const keep = page.getByRole("button", { name: S.keepMove });
  await expect(keep).toBeVisible();
  await timedClick(page, `flytt gruppe (${label})`, keep, "/(Gruppen er ferdig|Group done)/.test(document.body.innerText)");
}

test.describe.configure({ mode: "serial" });
test.setTimeout(240_000);

test.beforeAll(() => {
  if (process.platform !== "win32") throw new Error("Ekte-app-e2e kjører bare på Windows (WebView2, Windows Papirkurv).");
  if (!recycleGuardOk()) throw new Error("Vern: testene bruker Windows Papirkurv og appens datamappe. Kjør bare i CI (CI=true) eller sett E2E_ALLOW_RECYCLE=1.");
  armed = true;
  purgeTestFilesFromRecycleBin(); // deterministisk: gamle fjelltest-* fra tidligere kjøringer gir ellers falske treff
  root = mkdtempSync(join(tmpdir(), "fjell-real-"));
  manifest = generate(join(root, "lib"));
  homeA = homeDirs(join(root, "a"));
});

test.afterEach(async ({}, testInfo) => {
  if (testInfo.status !== testInfo.expectedStatus && app) {
    try {
      const png = testInfo.outputPath("feil.png");
      await app.page.screenshot({ path: png });
      await testInfo.attach("skjermbilde ved feil", { path: png, contentType: "image/png" });
      writeFileSync(join(OUT, `feil-${testInfo.title.replace(/\W+/g, "_").slice(0, 40)}.html`), await app.page.content());
    } catch { /* siden kan være borte */ }
  }
});

test.afterAll(async () => {
  await app?.close();
  const all = perf.map((p) => p.ms);
  const byName: Record<string, { n: number; p50: number | null; p95: number | null; max: number | null }> = {};
  for (const n of new Set(perf.map((p) => p.name))) {
    const xs = perf.filter((p) => p.name === n).map((p) => p.ms);
    byName[n] = { n: xs.length, p50: pct(xs, 0.5), p95: pct(xs, 0.95), max: Math.max(...xs) };
  }
  writeFileSync(join(OUT, "real-perf.json"), JSON.stringify({
    note: "Rapport, ikke porter. Kald start inkluderer CDP-tilkobling (øvre grense). Små utvalg: p95 er grovt.",
    platform: process.platform, node: process.version, date: new Date().toISOString(),
    coldStartMs: cold, actionsOverall: { n: all.length, p50: pct(all, 0.5), p95: pct(all, 0.95) }, actions: byName, samples: perf,
  }, null, 2));
  if (armed) purgeTestFilesFromRecycleBin();
  if (root) try { rmSync(root, { recursive: true, force: true }); } catch { /* ignorer */ }
});

test("1. førstegangs oppstart: onboarding, datamappen opprettes, skann, ekte miniatyrer", async () => {
  freshDataDir(homeA);
  expect(dataDirExists(homeA), "datamappen skal ikke finnes før første start").toBe(false);
  app = await launchApp(homeA);
  cold["førstegang"] = app.firstScreenMs;
  const page = app.page;
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByRole("button", { name: S.welcomeStart }), "første start skal vise onboarding").toBeVisible();
  await onboardAndScan(page, lib());
  // Tidligere ekte feil: datamappen ble ikke opprettet ved førstegangs start. Konfig skrives når første skann starter.
  await expect.poll(() => dataDirExists(homeA), { timeout: 15_000, message: `datamappen ble ikke opprettet (${dataDirs(homeA).join(" eller ")})` }).toBe(true);
  const n = await scanSettled(page);
  expect.soft(n, "forventet ett duplikat per par").toBe(manifest.dupGroups);

  await hash(page, "#/review/exact_dup");
  await expect(page.getByRole("link", { name: S.groupLink })).toHaveCount(manifest.dupGroups, { timeout: 30_000 });
  await page.getByRole("link", { name: S.groupLink }).first().click();
  await expect(page.getByRole("button", { name: S.keepMove })).toBeVisible();
  const imgs = page.locator("main img, img");
  await expect.poll(async () => imgs.count(), { message: "ingen miniatyrer i gruppen" }).toBeGreaterThanOrEqual(2);
  await expect.poll(
    () => imgs.evaluateAll((xs) => xs.filter((i) => (i as HTMLImageElement).naturalWidth > 0).length),
    { timeout: 30_000, message: "miniatyrene lastet ikke (naturalWidth == 0): ekte miniatyrer mangler" },
  ).toBeGreaterThanOrEqual(2);
});

test("2. flytt en duplikatgruppe, angre, flytt igjen (filer endres på disk)", async () => {
  const page = app!.page;
  expect(missingFiles(), "ingenting skal være flyttet ennå").toHaveLength(0);
  await hash(page, "#/review/exact_dup");
  await page.getByRole("link", { name: S.groupLink }).first().click();
  const keep = page.getByRole("button", { name: S.keepMove });
  await timedClick(page, "flytt gruppe (første)", keep, "/(Gruppen er ferdig|Group done)/.test(document.body.innerText)");
  await expect.poll(() => missingFiles().length, { message: "én fil skal være flyttet bort fra biblioteket" }).toBe(1);
  await timedClick(page, "angre gruppe", page.getByRole("button", { name: S.undo }), "!/(Gruppen er ferdig|Group done)/.test(document.body.innerText)");
  await expect.poll(() => missingFiles().length, { message: "angre skal legge filen tilbake" }).toBe(0);
  for (const f of manifest.files) expect(sha(abs(f.rel)), `${f.rel} endret etter angre`).toBe(f.sha256);
  await timedClick(page, "flytt gruppe (igjen)", page.getByRole("button", { name: S.keepMove }), "/(Gruppen er ferdig|Group done)/.test(document.body.innerText)");
  await expect.poll(() => missingFiles().length).toBe(1);
  // navigasjon (ytelsesutvalg)
  const nav = page.getByRole("navigation");
  for (let i = 0; i < 6; i++) {
    await timedClick(page, "nav Hjem", nav.getByRole("link", { name: S.navHome }).first(), "document.querySelector('h1')?.textContent !== before");
    await timedClick(page, "nav Papirkurv", nav.getByRole("link", { name: S.navTrash }).first(), "document.querySelector('h1')?.textContent !== before");
  }
});

test("3. omstart bevarer bibliotek og papirkurvtilstand", async () => {
  await app!.close();
  app = await launchApp(homeA);
  cold["omstart"] = app.firstScreenMs;
  const page = app.page;
  await expect(page.getByRole("button", { name: S.welcomeStart }), "onboarding skal ikke vises på nytt").toHaveCount(0);
  await expect(page.getByText(S.folderCurrent(lib())).first(), "biblioteket skal være husket").toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("link", { name: S.homeTrashLink })).toContainText(/\(1\)/, { timeout: 30_000 });
  expect(missingFiles(), "flyttet fil skal fortsatt være borte fra biblioteket").toHaveLength(1);
  await hash(page, "#/trash");
  await expect(page.getByRole("button", { name: S.trashEmpty })).toBeVisible();
});

test("4. Tøm til Windows Papirkurv: filene ligger der, beholdte originaler er uendret", async () => {
  const page = app!.page;
  for (let i = 1; i < manifest.dupGroups; i++) await commitFirstGroup(page, `gruppe ${i + 1}`);
  await expect.poll(() => missingFiles().length, { message: "alle duplikatgrupper skal ha flyttet én fil" }).toBe(manifest.dupGroups);
  await hash(page, "#/trash");
  await timedClick(page, "åpne Tøm-dialog", page.getByRole("button", { name: S.trashEmpty }), "!!document.querySelector('dialog[open]')");
  const dlg = page.getByRole("dialog");
  await dlg.getByRole("radio", { name: S.modeRecycle }).check();
  await timedClick(page, "tøm til Papirkurv", dlg.getByRole("button", { name: S.continue_ }), "/(Flyttet til Windows Papirkurv|Moved to the Windows Recycle Bin)/.test(document.body.innerText)");
  await expect(page.getByText(S.purgedRecycle).first()).toBeVisible({ timeout: 60_000 });

  const gone = missingFiles();
  expect(gone).toHaveLength(manifest.dupGroups);
  const bin = recycleBinNames();
  for (const f of gone) {
    const base = f.rel.split("/").pop()!.replace(/\.[^.]+$/, "");
    expect(bin.some((n) => n.startsWith(base)), `«${base}» ligger ikke i Windows Papirkurv. Papirkurv har: ${bin.filter((n) => n.startsWith("fjelltest-")).join(", ") || "(ingen testfiler)"}`).toBe(true);
  }
  // Alle andre filer (beholdte duplikater, unike, uskarp, skjermbilde, korrupt) er urørt: samme SHA-256
  for (const f of manifest.files.filter((x) => !gone.includes(x))) {
    expect(sha(abs(f.rel)), `${f.rel} er endret`).toBe(f.sha256);
  }
  // Hver gruppe har nøyaktig ett beholdt medlem
  for (let g = 1; g <= manifest.dupGroups; g++) expect(manifest.files.filter((f) => f.group === `dup${g}` && existsSync(abs(f.rel)))).toHaveLength(1);
});

test("5. skymappe (sti med OneDrive): advarsel vises og papirkurv-handling er sperret", async () => {
  await app!.close();
  const cloudRoot = join(root, "OneDrive", "Bilder");
  mkdirSync(cloudRoot, { recursive: true });
  const cloudLib = join(cloudRoot, "photos");
  const m2 = generate(join(root, "lib2"));
  cpSync(m2.photosDir, cloudLib, { recursive: true });
  const homeB = homeDirs(join(root, "b"));
  freshDataDir(homeB); // ny førstegangs start (ellers husker appen bibliotek A hvis Tauri ignorerer APPDATA-env)
  app = await launchApp(homeB);
  const page = app.page;
  await onboardAndScan(page, cloudLib);
  await scanSettled(page);
  await hash(page, "#/review/exact_dup");
  await expect(page.getByText(S.cloudOneDrive).first(), "skymappe-advarsel mangler").toBeVisible({ timeout: 30_000 });
  const moveBtns = page.getByRole("button", { name: S.moveBar });
  for (const b of await moveBtns.all()) await expect(b, "«Flytt til papirkurv» skal være sperret i skymappe").toBeDisabled();
  const link = page.getByRole("link", { name: S.groupLink }).first();
  if (await link.count()) {
    await link.click();
    const keep = page.getByRole("button", { name: S.keepMove });
    if (await keep.count() && (await keep.isEnabled())) await keep.click(); // eventuelt klikk gir feilmelding, men ingenting flyttes
  }
  await page.waitForTimeout(1500);
  for (const f of m2.files) expect(sha(join(cloudLib, f.rel)), `${f.rel}: skymappe-sperren holdt ikke`).toBe(f.sha256);
});
