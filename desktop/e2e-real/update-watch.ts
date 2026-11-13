// Hjelpere for oppdateringstesten (update.spec.ts): overvåker, prosess- og versjonssjekker, lokal oppdateringsserver. Kun Windows.
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { powershell } from "./ps";

const here = dirname(fileURLToPath(import.meta.url));
export const WATCH_SCRIPT = join(here, "../scripts/update-e2e/watch.ps1");
export const SERVE_SCRIPT = join(here, "../scripts/update-e2e/serve.mjs");

/** Tekst i installervinduer (UIA-linjer i overvåkerloggen) som betyr at installasjonen er avbrutt. */
export const FAIL_RE = /avbrutt|aborted|kunne ikke avslutte|kan ikke avslutte|could not (close|quit|exit|terminate)|unable to (close|quit|exit)|failed to (close|kill|quit|terminate)|error opening file|feil ved/i;

const q = (s: string) => s.replace(/'/g, "''");
export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function startWatcher(log: string): ChildProcess {
  return spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", WATCH_SCRIPT, "-Log", log], { stdio: "ignore", windowsHide: true });
}
export function stopWatcher(w: ChildProcess | undefined): void {
  if (w?.pid) spawn("taskkill", ["/PID", String(w.pid), "/T", "/F"], { stdio: "ignore" });
}
export const readLog = (log: string): string => (existsSync(log) ? readFileSync(log, "utf8") : "");
/** Første UIA-linje i loggen som ser ut som et feilvindu, ellers null. */
export const failureWindow = (log: string): string | null => readLog(log).split("\n").find((l) => l.includes(" UIA ") && FAIL_RE.test(l)) ?? null;
/** Linje der installeren ble startet med /UPDATE (kommandolinjen), ellers null. */
export const installerStartLine = (log: string): string | null => readLog(log).split("\n").find((l) => l.includes(" + pid=") && /\/UPDATE/i.test(l)) ?? null;

export function isAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true; } catch { return false; }
}

/** Installer-prosesser (kommandolinje med /UPDATE, eller navn som Fjell*setup*). */
export function installerProcs(): { pid: number; name: string; cmd: string }[] {
  const out = powershell("$r = @(Get-CimInstance Win32_Process | Where-Object { $_.ProcessId -ne $PID -and $_.Name -notmatch '^(powershell|pwsh)' -and ($_.CommandLine -match '/UPDATE' -or $_.Name -like 'Fjell*setup*') } | ForEach-Object { [pscustomobject]@{ pid = [int]$_.ProcessId; name = $_.Name; cmd = [string]$_.CommandLine } }); ConvertTo-Json -InputObject $r -Compress");
  return out ? (JSON.parse(out) as { pid: number; name: string; cmd: string }[]) : [];
}
export function procsByPath(exe: string): number[] {
  const out = powershell(`$r = @(Get-Process | Where-Object { $_.Path -eq '${q(exe)}' } | ForEach-Object { $_.Id }); ConvertTo-Json -InputObject $r -Compress`);
  return out ? (JSON.parse(out) as number[]) : [];
}
export function killByPath(exe: string): void {
  powershell(`Get-Process | Where-Object { $_.Path -eq '${q(exe)}' } | Stop-Process -Force -ErrorAction SilentlyContinue`);
}
export function killInstallers(): void {
  powershell("Get-CimInstance Win32_Process | Where-Object { $_.ProcessId -ne $PID -and $_.Name -notmatch '^(powershell|pwsh)' -and ($_.CommandLine -match '/UPDATE' -or $_.Name -like 'Fjell*setup*') } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }");
}

/** Produkt-/filversjon fra exe-ens versjonsressurs (f.eks. «0.0.2» eller «0.0.2.0»). */
export function exeVersion(exe: string): string {
  return powershell(`(Get-Item -LiteralPath '${q(exe)}').VersionInfo.ProductVersion`);
}
/** DisplayVersion for «Fjell» i avinstallasjonsnøklene (HKCU og HKLM). Tom liste hvis ingen. */
export function registryVersions(): string[] {
  const out = powershell("$r = @(Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*','HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'Fjell' } | ForEach-Object { [string]$_.DisplayVersion }); ConvertTo-Json -InputObject $r -Compress");
  return out ? (JSON.parse(out) as string[]) : [];
}

/** Starter lokal HTTP-server (serve.mjs) og venter til latest.json svarer. */
export async function startServer(www: string, port: number, log: string): Promise<ChildProcess> {
  const srv = spawn(process.execPath, [SERVE_SCRIPT, www, String(port), log], { stdio: "ignore" });
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try { if ((await fetch(`http://127.0.0.1:${port}/latest.json`)).ok) return srv; } catch { /* ikke klar */ }
    await sleep(200);
  }
  srv.kill();
  throw new Error(`Oppdateringsserveren svarte ikke på 127.0.0.1:${port}/latest.json innen 15 s.`);
}
