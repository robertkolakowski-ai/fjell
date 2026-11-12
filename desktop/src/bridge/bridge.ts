// Velger bro: Tauri når `__TAURI_INTERNALS__` finnes, ellers MockBridge (nettleser/dev).
import type { CoreBridge } from "./core-bridge";
import { MockBridge } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";

export function createBridge(): CoreBridge {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? new TauriBridge()
    : new MockBridge({ delayMs: 150 });
}

const bridge = createBridge();
export const getState = () => bridge.getState();
export type { CoreBridge };
