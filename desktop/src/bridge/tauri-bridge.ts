// TauriBridge: invoke mot kommandoene i desktop/src-tauri (fjell-shell). Grensesnittet er uendret.
import { invoke } from "@tauri-apps/api/core";
import type { CoreBridge } from "./core-bridge";
import type { AppState, CardPage, Decision, EmptyResult, KindCode, PermanentAck } from "./types";

export type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

export class TauriBridge implements CoreBridge {
  constructor(private readonly call: Invoke = invoke) {}

  getState(): Promise<AppState> {
    return this.call<AppState>("get_state");
  }
  startScan(full: boolean): Promise<void> {
    return this.call<void>("start_scan", { full });
  }
  listCards(kind: KindCode, limit: number, cursor: number | null = null): Promise<CardPage> {
    return this.call<CardPage>("list_cards", { kind, limit, cursor });
  }
  decide(ids: number[], decision: Decision): Promise<number> {
    return this.call<number>("decide_cards", { ids, decision });
  }
  setKeeper(group: number, asset: number): Promise<void> {
    return this.call<void>("set_keeper", { group, keeper: asset });
  }
  undo(): Promise<number> {
    return this.call<number>("undo");
  }
  /** «Tøm» via Papirkurv (standard). Permanent sletting har egen bekreftelse og ligger ikke her. */
  async purge(): Promise<number> {
    const r = await this.call<{ purged: number }>("empty_trash");
    return r.purged;
  }
  emptyTrash(permanent: PermanentAck | null = null): Promise<EmptyResult> {
    return this.call<EmptyResult>("empty_trash", { permanent });
  }
}
