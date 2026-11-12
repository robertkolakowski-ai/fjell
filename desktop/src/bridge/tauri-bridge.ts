// TauriBridge: invoke mot eksisterende kommandoer i desktop/src-tauri (uendret Rust).
import { invoke } from "@tauri-apps/api/core";
import type { CoreBridge } from "./core-bridge";
import type { AppState, BridgeError, CardPage, Decision, KindCode } from "./types";

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
  undo(): Promise<number> {
    return this.call<number>("undo");
  }
  purge(): Promise<number> {
    const e: BridgeError = { code: "unsupported", detail: "purge har ingen Tauri-kommando ennå" };
    return Promise.reject(e);
  }
}
