// Ekte oppdatering via appen selv (Tauri updater -> NSIS /P /UPDATE -> app avslutter seg selv). Kun Windows, kun med FJELL_UPDATE_E2E=1.
// Reproduserer brukerfeilen «Installasjonen er avbrutt – Kunne ikke avslutte Fjell». Se docs/mvp/OPPDATERING-E2E.md.
// Forutsetter (settes av .github/workflows/update-e2e.yml): v1 er installert, FJELL_EXE peker på den, og FJELL_UPDATE_WWW har latest.json + setup.exe for v2.
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import type { ChildProcess } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { homeDirs, launchApp, type App } from "./launch";
import { exeVersion, failureWindow, installerProcs, installerStartLine, isAlive, killByPath, killInstallers, procsByPath, readLog, registryVersions, sleep, startServer, startWatcher, stopWatcher } from "./update-watch";

const ENABLED = process.env.FJELL_UPDATE_E2E === "1";
const need = (k: string): string => { const v = process.env[k]; if (!v) throw new Error(`Mangler miljøvariabel ${k} (se docs/mvp/OPPDATERING-E2E.md).`); return v; };
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const T = {
  check: /^(Se etter oppdatering|Check for update)$/,
  install: /^(Installer og start på nytt|Install and restart)$/,
  heading: /^(Oppdateringer|Updates)$/,
  available: (v: string) => new RegExp(`(Ny versjon ${esc(v)} er tilgjengelig|Version ${esc(v)} is available)`),
  version: (v: string) => new RegExp(`^(Versjon|Version) ${esc(v)}$`),
};
const INSTALL_TIMEOUT_MS = Number(process.env.FJELL_UPDATE_TIMEOUT_MS ?? 180_000);

test.describe("oppdatering via appen", () => {
  test.skip(!ENABLED, "Kjøres bare av update-e2e.yml (FJELL_UPDATE_E2E=1).");
  test.describe.configure({ mode: "serial" });

  let out = "", root = "", watchLog = "";
  let server: ChildProcess | undefined, watcher: ChildProcess | undefined;
  let app: App | undefined;
  const exe = () => need("FJELL_EXE");

  test.beforeAll(async () => {
    if (process.platform !== "win32") throw new Error("Oppdateringstesten kjører bare på Windows.");
    if (process.env.CI !== "true") throw new Error("Vern: testen installerer/avslutter prosesser. Kjør bare i CI (CI=true).");
    out = process.env.FJELL_UPDATE_OUT ?? join(tmpdir(), "fjell-update-out");
    mkdirSync(out, { recursive: true });
    root = mkdtempSync(join(tmpdir(), "fjell-upd-"));
    watchLog = join(out, "watch.log");
    server = await startServer(need("FJELL_UPDATE_WWW"), Number(need("FJELL_UPDATE_PORT")), join(out, "server.log"));
  });

  test.afterAll(async () => {
    await app?.close().catch(() => undefined);
    killInstallers();
    stopWatcher(watcher);
    server?.kill();
  });

  test.afterEach(async ({}, testInfo) => {
    if (testInfo.status !== testInfo.expectedStatus && app) {
      try { await app.page.screenshot({ path: join(out, "feil-app.png") }); } catch { /* siden kan være borte */ }
    }
  });

  async function openSettings(page: Page): Promise<void> {
    // Førstegangs-onboarding kan sende oss tilbake til Hjem like etter oppstart; prøv på nytt.
    for (let i = 0; i < 6; i++) {
      await page.evaluate(() => { location.hash = "#/settings"; });
      try { await page.getByRole("heading", { level: 2, name: T.heading }).waitFor({ timeout: 3000 }); return; } catch { /* prøv igjen */ }
    }
    throw new Error("Fant ikke Innstillinger med oppdateringsseksjon.");
  }

  test("1. installert v1, sjekk, last ned, installer og avslutt: installeren fullfører uten feilvindu, ny versjon, appen starter igjen", async () => {
    const from = need("FJELL_UPDATE_FROM"), to = need("FJELL_UPDATE_TO");
    const installed = exe();

    // Forutsetning: v1 ligger på disk og er den eneste versjonen.
    expect(exeVersion(installed), "installert exe skal være startversjonen").toMatch(new RegExp(`^${esc(from)}`));

    watcher = startWatcher(watchLog);
    await sleep(1500);

    app = await launchApp(homeDirs(join(root, "a")));
    const page = app.page;
    const oldPid = app.pid;
    await openSettings(page);
    await expect(page.getByText(T.version(from)).first(), "appen skal vise startversjonen").toBeVisible();

    await page.getByRole("button", { name: T.check }).click();
    await expect(page.getByText(T.available(to)).first(), "sjekken mot lokal server skal finne ny versjon").toBeVisible({ timeout: 30_000 });

    // Fra nå av kan siden forsvinne når appen avslutter seg selv; alt videre følges utenfra.
    const t0 = Date.now();
    await page.getByRole("button", { name: T.install }).click({ noWaitAfter: true, timeout: 10_000 }).catch(() => undefined);

    // 1) Gammel prosess skal avslutte (appen kaller exit(0) etter å ha startet installeren).
    let oldGoneMs: number | null = null;
    let sawInstaller = false;
    let failure: string | null = null;
    while (Date.now() - t0 < INSTALL_TIMEOUT_MS) {
      failure = failureWindow(watchLog);
      if (failure) break;
      if (!sawInstaller && installerStartLine(watchLog)) sawInstaller = true;
      if (oldGoneMs === null && !isAlive(oldPid)) oldGoneMs = Date.now() - t0;
      if (oldGoneMs !== null && sawInstaller && installerProcs().length === 0 && exeVersion(installed).startsWith(to)) break;
      await sleep(1000);
    }
    const timeline = readLog(watchLog);
    writeFileSync(join(out, "watch-sammendrag.txt"), `gammel prosess borte: ${oldGoneMs ?? "nei"} ms\ninstaller sett: ${sawInstaller}\nfeilvindu: ${failure ?? "nei"}\n`);

    expect(failure, `NSIS viste feilvindu (brukerfeilen er reprodusert):\n${failure}\n\n--- watch.log ---\n${timeline.slice(-4000)}`).toBeNull();
    expect(sawInstaller, `installeren ble aldri startet med /UPDATE\n${timeline.slice(-3000)}`).toBe(true);
    expect(installerStartLine(watchLog), "installeren skal startes med /P og /UPDATE (passive)").toMatch(/\/P\b/i);
    expect(oldGoneMs, `gammel appprosess (pid ${oldPid}) var ikke borte etter ${INSTALL_TIMEOUT_MS / 1000} s\n${timeline.slice(-3000)}`).not.toBeNull();
    const left = installerProcs();
    expect(left, `installeren henger fortsatt etter ${INSTALL_TIMEOUT_MS / 1000} s: ${JSON.stringify(left)}\n${timeline.slice(-3000)}`).toEqual([]);

    // 2) Ny versjon på disk (exe-ressurs og avinstallasjonsnøkkel).
    expect(exeVersion(installed), "installert exe skal ha ny versjon").toMatch(new RegExp(`^${esc(to)}`));
    const reg = registryVersions();
    if (reg.length) expect(reg.every((v) => v.startsWith(to)), `registry DisplayVersion: ${JSON.stringify(reg)}`).toBe(true);

    // 3) Installeren kan ha startet appen på nytt (arver CDP-port). Avslutt den, og start som bruker ville gjort.
    await sleep(3000);
    const relaunched = procsByPath(installed).length > 0;
    writeFileSync(join(out, "relaunch.txt"), `installeren startet appen på nytt: ${relaunched}\n`);
    killByPath(installed);
    for (let i = 0; i < 30 && procsByPath(installed).length; i++) await sleep(500);
    await app.close().catch(() => undefined);
    app = undefined;

    app = await launchApp(homeDirs(join(root, "b")));
    await openSettings(app.page);
    await expect(app.page.getByText(T.version(to)).first(), "ny app skal vise ny versjon").toBeVisible();
    await app.close();
    app = undefined;
  });
});
