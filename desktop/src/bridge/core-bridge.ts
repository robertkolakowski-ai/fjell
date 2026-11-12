// CoreBridge: felles grensesnitt for Tauri og mock. All UI går via dette (ingen direkte invoke).
import type { AppState, CardPage, Decision, EmptyResult, KindCode, PermanentAck } from "./types";

export interface CoreBridge {
  getState(): Promise<AppState>;
  startScan(full: boolean): Promise<void>;
  listCards(kind: KindCode, limit: number, cursor?: number | null): Promise<CardPage>;
  /** Papirkurv/behold. Returnerer batch-id (ett angre). Aldri hard sletting. */
  decide(ids: number[], decision: Decision): Promise<number>;
  /** Bytter keeper i en gruppe (K2). `asset` må være gruppens keeper eller et av gruppens forslag. Ingen filer røres. */
  setKeeper(group: number, asset: number): Promise<void>;
  /** Angrer siste batch. Returnerer batch-id. */
  undo(): Promise<number>;
  /** Tømmer «Nylig slettet» via Papirkurv. Returnerer antall tømt. */
  purge(): Promise<number>;
  /** «Tøm» med valg: Windows Papirkurv (standard) eller permanent (krever `PermanentAck`: avkrysning + riktig antall). */
  emptyTrash(permanent?: PermanentAck | null): Promise<EmptyResult>;
}
