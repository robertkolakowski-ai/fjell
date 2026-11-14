// Velger bro: Tauri når `__TAURI_INTERNALS__` finnes, ellers MockBridge (nettleser/dev).
import type { CoreBridge } from "./core-bridge";
import { MOCK_UPDATES, MockBridge, type MockUpdate } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";
import { THUMB_ERROR_CODES, type ThumbErrorCode } from "./types";

export function createBridge(): CoreBridge {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? new TauriBridge()
    : new MockBridge({
      delayMs: mockDelay(),
      update: mockUpdate(),
      scanPolls: mockScanPolls(),
      autoUpdate: mockAuto(),
      pick: mockPick(),
      ...mockUnsupported(),
      ...mockLibrary(),
      ...mockThumbs(),
    });
}

/** Mappevalg i mock (bare dev-server): `?mockPick=C:\\Bilder\\Ferie` gir den stien, `?mockPick=cancel` simulerer avbrutt. Uten parameter er mappevalg utilgjengelig. */
function mockPick(): string | null | undefined {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockPick");
    if (v !== null) return v === "cancel" ? null : v;
  }
  return undefined;
}

/** Simulert latens i nettleser/dev. `?mockDelay=0` (bare i dev-server) gir e2e- og ytelsesmålinger uten kunstig ventetid. */
function mockDelay(): number {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockDelay");
    if (v !== null && Number.isFinite(Number(v))) return Number(v);
  }
  return 150;
}

/** Dev-parametre for e2e (M9), bare i dev-server: `?mockUpdate=none|available|error|slow`, `?mockScanPolls=N` (skann blir stående i «analyserer») og `?mockAuto=1` («Sjekk automatisk ved oppstart» på). */
function mockUpdate(): MockUpdate {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockUpdate");
    if (MOCK_UPDATES.includes(v as MockUpdate)) return v as MockUpdate;
  }
  return "none";
}
function mockAuto(): boolean {
  return !!import.meta.env.DEV && typeof location !== "undefined" && new URLSearchParams(location.search).get("mockAuto") === "1";
}
function mockScanPolls(): number {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockScanPolls");
    if (v !== null && Number.isInteger(Number(v)) && Number(v) >= 0) return Number(v);
  }
  return 0;
}

/** Bare dev: `?mockUnsupported=N` gir N filer uten Papirkurv-støtte, `all` gir volum uten Papirkurv (e2e og skjermbilder). */
function mockUnsupported(): { noBin?: boolean; unsupportedCount?: number } {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockUnsupported");
    if (v === "all") return { noBin: true };
    if (v !== null && Number.isInteger(Number(v)) && Number(v) > 0) return { unsupportedCount: Number(v) };
  }
  return {};
}

/** Bare dev: `?mockLibrary=<sti>` setter valgt mappe i mocken (e2e og skjermbilder av skymappe-advarselen, f.eks. `C:\\Users\\a\\OneDrive\\Bilder`). */
function mockLibrary(): { library?: string } {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockLibrary");
    if (v !== null && v.trim() !== "") return { library: v };
  }
  return {};
}

/** Bare dev: `?mockThumbErr=3:corrupt,4:unavailable` gir miniatyrfeil per bilde-id; `?mockThumbDelay=N` setter ventetid per miniatyr (ms). */
function mockThumbs(): { thumbErrors?: Record<number, ThumbErrorCode>; thumbDelayMs?: number } {
  const out: { thumbErrors?: Record<number, ThumbErrorCode>; thumbDelayMs?: number } = {};
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const q = new URLSearchParams(location.search);
    const errs: Record<number, ThumbErrorCode> = {};
    for (const part of (q.get("mockThumbErr") ?? "").split(",")) {
      const [id, code] = part.split(":");
      if (Number.isInteger(Number(id)) && (THUMB_ERROR_CODES as readonly string[]).includes(code)) errs[Number(id)] = code as ThumbErrorCode;
    }
    if (Object.keys(errs).length > 0) out.thumbErrors = errs;
    const d = q.get("mockThumbDelay");
    if (d !== null && Number.isFinite(Number(d))) out.thumbDelayMs = Number(d);
  }
  return out;
}

/** Appens bridge: Tauri når tilgjengelig, ellers MockBridge (dev/test). */
export const bridge = createBridge();
export const getState = () => bridge.getState();
export type { CoreBridge };
