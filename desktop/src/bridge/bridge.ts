// Velger bro: Tauri når `__TAURI_INTERNALS__` finnes, ellers MockBridge (nettleser/dev).
import type { CoreBridge } from "./core-bridge";
import { MockBridge } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";

export function createBridge(): CoreBridge {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? new TauriBridge()
    : new MockBridge({ delayMs: mockDelay(), ...mockUnsupported(), ...mockLibrary() });
}

/** Simulert latens i nettleser/dev. `?mockDelay=0` (bare i dev-server) gir e2e- og ytelsesmålinger uten kunstig ventetid. */
function mockDelay(): number {
  if (import.meta.env.DEV && typeof location !== "undefined") {
    const v = new URLSearchParams(location.search).get("mockDelay");
    if (v !== null && Number.isFinite(Number(v))) return Number(v);
  }
  return 150;
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

/** Appens bridge: Tauri når tilgjengelig, ellers MockBridge (dev/test). */
export const bridge = createBridge();
export const getState = () => bridge.getState();
export type { CoreBridge };
