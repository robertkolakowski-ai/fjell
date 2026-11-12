// Tynt lag over Tauri `invoke` (CoreBridge-embryo, ARKITEKTUR beslutning 11).
// Grensesnitt: getState(). Ingen forretningslogikk; UI-fasen (M6) utvider og legger til MockBridge.
import { invoke } from "@tauri-apps/api/core";
import type { AppState } from "./types";

export function getState(): Promise<AppState> {
  return invoke<AppState>("get_state");
}
