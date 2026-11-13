// Starter den bygde Tauri-appen (fjell-desktop.exe) med isolert APPDATA/LOCALAPPDATA og koble Playwright til WebView2 over CDP.
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser, type Page } from "@playwright/test";

const here = dirname(fileURLToPath(import.meta.url));
export const DATA_IDENTIFIER = "no.fjell.desktop"; // tauri.conf.json: identifier (app_data_dir = %APPDATA%\<identifier>)

export function exePath(): string {
  const p = process.env.FJELL_EXE ?? resolve(here, "../src-tauri/target/release/fjell-desktop.exe");
  if (!existsSync(p)) throw new Error(`Fant ikke appen: ${p}. Bygg først (npx tauri build --no-bundle) eller sett FJELL_EXE.`);
  return p;
}

export interface Home { appdata: string; localappdata: string }
export interface App {
  page: Page;
  /** ms fra prosessstart til første h1 er synlig (øvre grense: inkluderer CDP-tilkobling, polling ca. 50–100 ms). */
  firstScreenMs: number;
  pid: number;
  close(): Promise<void>;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function killTree(pid: number): void {
  if (process.platform === "win32") spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { stdio: "ignore" });
  else { try { process.kill(pid, "SIGKILL"); } catch { /* allerede borte */ } }
}

export async function launchApp(home: Home, port = Number(process.env.FJELL_CDP_PORT ?? 9222)): Promise<App> {
  mkdirSync(home.appdata, { recursive: true });
  mkdirSync(home.localappdata, { recursive: true });
  const t0 = Date.now();
  const proc: ChildProcess = spawn(exePath(), [], {
    stdio: "ignore",
    windowsHide: false,
    env: {
      ...process.env,
      APPDATA: home.appdata,
      LOCALAPPDATA: home.localappdata,
      // NB: Tauri/dirs bruker SHGetKnownFolderPath, som ikke leser APPDATA/LOCALAPPDATA. Env-overstyringen over er derfor ikke
      // en garanti for app_data_dir (spec-filen rydder og sjekker begge steder). WebView2 respekterer derimot denne:
      WEBVIEW2_USER_DATA_FOLDER: join(home.localappdata, "EBWebView"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
  });
  let exited: number | null | undefined;
  proc.once("exit", (code) => { exited = code; });
  const pid = proc.pid;
  if (!pid) throw new Error("Kunne ikke starte appen (ingen pid).");

  let browser: Browser | undefined;
  const closeAll = async () => {
    try { await browser?.close(); } catch { /* ignorer */ }
    killTree(pid);
    for (let i = 0; i < 50 && exited === undefined; i++) await sleep(100);
  };
  try {
    const deadline = Date.now() + 60_000;
    let lastErr = "";
    while (Date.now() < deadline) {
      if (exited !== undefined) throw new Error(`Appen avsluttet av seg selv under oppstart (kode ${exited}).`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 3000 }); break; }
      catch (e) { lastErr = String(e); await sleep(100); }
    }
    if (!browser) throw new Error(`Fikk ikke CDP-forbindelse til 127.0.0.1:${port} innen 60 s. Siste feil: ${lastErr}`);
    let page: Page | undefined;
    while (Date.now() < deadline && !page) {
      page = browser.contexts().flatMap((c) => c.pages()).find((p) => /tauri\.localhost|^tauri:/.test(p.url()) || (p.url() !== "about:blank" && p.url() !== ""));
      if (!page) await sleep(100);
    }
    if (!page) throw new Error("Fant ikke appvinduet (WebView2-siden) over CDP.");
    await page.locator("h1").first().waitFor({ timeout: Math.max(1000, deadline - Date.now()) });
    return { page, firstScreenMs: Date.now() - t0, pid, close: closeAll };
  } catch (e) {
    await closeAll();
    throw e;
  }
}

export const homeDirs = (root: string): Home => ({ appdata: join(root, "appdata"), localappdata: join(root, "localappdata") });
