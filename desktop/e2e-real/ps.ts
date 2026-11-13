// PowerShell-hjelpere: les Windows Papirkurv via Shell.Application (Namespace 10 = Papirkurv).
import { spawnSync } from "node:child_process";

/** Vern: kun CI, eller eksplisitt opt-in lokalt. Testene flytter/sletter filer i Windows Papirkurv. */
export function recycleGuardOk(): boolean {
  return process.env.CI === "true" || process.env.E2E_ALLOW_RECYCLE === "1";
}

export function powershell(script: string): string {
  const r = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script], { encoding: "utf8", timeout: 60_000 });
  if (r.status !== 0) throw new Error(`PowerShell feilet (kode ${r.status}): ${r.stderr || r.stdout}`);
  return r.stdout.trim();
}

/** Visningsnavn (kan mangle filendelse, avhengig av Utforsker-innstilling) på alt i Windows Papirkurv. */
export function recycleBinNames(): string[] {
  const out = powershell("$items = @((New-Object -ComObject Shell.Application).Namespace(10).Items() | ForEach-Object { $_.Name }); ConvertTo-Json -InputObject $items -Compress");
  if (!out) return [];
  const v = JSON.parse(out) as unknown;
  return Array.isArray(v) ? (v as string[]) : [String(v)];
}

/** Rydder våre testfiler (prefiks «fjelltest-») ut av Papirkurven, så lokale kjøringer ikke samler søppel. Best effort. */
export function purgeTestFilesFromRecycleBin(): void {
  if (!recycleGuardOk()) return; // aldri rør brukerens Papirkurv uten vern; bare prefiks «fjelltest-», aldri tømming av hele kurven
  try {
    powershell("(New-Object -ComObject Shell.Application).Namespace(10).Items() | Where-Object { $_.Name -like 'fjelltest-*' } | ForEach-Object { Remove-Item -LiteralPath $_.Path -Recurse -Force -ErrorAction SilentlyContinue }");
  } catch { /* ignorer */ }
}
