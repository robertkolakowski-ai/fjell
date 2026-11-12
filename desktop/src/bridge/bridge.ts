// Velger bro: Tauri når `__TAURI_INTERNALS__` finnes, ellers MockBridge (nettleser/dev).
import type { CoreBridge } from "./core-bridge";
import { MockBridge } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";

export function createBridge(): CoreBridge {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? new TauriBridge()
    : new MockBridge({ delayMs: mockDelay(), pick: mockPick() });
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

/** Appens bridge: Tauri når tilgjengelig, ellers MockBridge (dev/test). */
export const bridge = createBridge();
export const getState = () => bridge.getState();
export type { CoreBridge };
