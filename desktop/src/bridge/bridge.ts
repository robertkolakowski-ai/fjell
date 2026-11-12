// Velger bro: Tauri når `__TAURI_INTERNALS__` finnes, ellers MockBridge (nettleser/dev).
import type { CoreBridge } from "./core-bridge";
import { MOCK_UPDATES, MockBridge, type MockUpdate } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";

export function createBridge(): CoreBridge {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? new TauriBridge()
    : new MockBridge({ delayMs: mockDelay(), update: mockUpdate(), scanPolls: mockScanPolls(), autoUpdate: mockAuto() });
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

/** Appens bridge: Tauri når tilgjengelig, ellers MockBridge (dev/test). */
export const bridge = createBridge();
export const getState = () => bridge.getState();
export type { CoreBridge };
