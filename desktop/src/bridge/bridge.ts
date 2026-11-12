// Velger bro: Tauri når `__TAURI_INTERNALS__` finnes, ellers MockBridge (nettleser/dev).
import type { CoreBridge } from "./core-bridge";
import { MockBridge } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";

export function createBridge(): CoreBridge {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? new TauriBridge()
    : new MockBridge({ delayMs: mockDelay() });
}

/** Simulert latens i nettleser/dev. `?mockDelay=0` (bare i dev-server) gir e2e- og ytelsesmålinger uten kunstig ventetid. */
function mockDelay(): number {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockDelay");
    if (v !== null && Number.isFinite(Number(v))) return Number(v);
  }
  return 150;
}

/** Appens bridge: Tauri når tilgjengelig, ellers MockBridge (dev/test). */
export const bridge = createBridge();
export const getState = () => bridge.getState();
export type { CoreBridge };
