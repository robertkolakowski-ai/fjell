// TauriBridge: invoke mot kommandoene i desktop/src-tauri (fjell-shell). Grensesnittet er uendret.
import { invoke } from "@tauri-apps/api/core";
import type { CoreBridge } from "./core-bridge";
import type {
  AppSettings, AppState, CardPage, Decision, EmptyResult, KindCode, PermanentAck, RestoreResult, TrashedItem,
} from "./types";

export type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

/** Åpner systemets mappevalg (tauri-plugin-dialog). Plugin lastes først ved bruk. null = avbrutt. */
export type PickDirectory = (title?: string) => Promise<string | null>;

const pickDirectory: PickDirectory = async (title) => {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === "string" ? r : null;
};

export class TauriBridge implements CoreBridge {
  readonly canPickFolder = true;

  constructor(private readonly call: Invoke = invoke, private readonly pick: PickDirectory = pickDirectory) {}

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
  listTrashed(): Promise<TrashedItem[]> {
    return this.call<TrashedItem[]>("list_trashed");
  }
  restore(assets: number[]): Promise<RestoreResult> {
    return this.call<RestoreResult>("restore_trashed", { ids: assets });
  }
  restoreAll(): Promise<RestoreResult> {
    return this.call<RestoreResult>("restore_all_trashed");
  }
  pauseScan(): Promise<void> {
    return this.call<void>("pause_scan");
  }
  resumeScan(): Promise<void> {
    return this.call<void>("resume_scan");
  }
  cancelScan(): Promise<void> {
    return this.call<void>("cancel_scan");
  }
  getLibrary(): Promise<string | null> {
    return this.call<string | null>("get_library");
  }
  pickFolder(title?: string): Promise<string | null> {
    return this.pick(title);
  }
  async setLibrary(path: string): Promise<void> {
    await this.call("set_library", { path });
  }
  getFirstRun(): Promise<boolean> {
    return this.call<boolean>("get_first_run");
  }
  completeOnboarding(): Promise<void> {
    return this.call<void>("complete_onboarding");
  }
  getSettings(): Promise<AppSettings> {
    return this.call<AppSettings>("get_settings");
  }
  setTrashDays(days: number): Promise<AppSettings> {
    return this.call<AppSettings>("set_trash_days", { days });
  }
  getLanguage(): Promise<string | null> {
    return this.call<string | null>("get_language");
  }
  setLanguage(language: string): Promise<string> {
    return this.call<string>("set_language", { language });
  }
}
