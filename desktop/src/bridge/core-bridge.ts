// CoreBridge: felles grensesnitt for Tauri og mock. All UI går via dette (ingen direkte invoke).
import type { AppState, CardPage, Decision, KindCode } from "./types";

export interface CoreBridge {
  getState(): Promise<AppState>;
  startScan(full: boolean): Promise<void>;
  listCards(kind: KindCode, limit: number, cursor?: number | null): Promise<CardPage>;
  /** Papirkurv/behold. Returnerer batch-id (ett angre). Aldri hard sletting. */
  decide(ids: number[], decision: Decision): Promise<number>;
  /** Angrer siste batch. Returnerer batch-id. */
  undo(): Promise<number>;
  /** Tømmer «Nylig slettet». Ingen Tauri-kommando ennå: TauriBridge avviser med `unsupported`. */
  purge(): Promise<number>;
}
