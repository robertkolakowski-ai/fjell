// TauriBridge: invoke mot kommandoene i desktop/src-tauri (fjell-shell). Grensesnittet er uendret.
import { invoke } from "@tauri-apps/api/core";
import type { CoreBridge } from "./core-bridge";
import type {
  AppSettings, AppState, CardPage, Decision, EmptyResult, KindCode, LibraryRoot, PermanentAck, RestoreResult, ThumbDto, Thumbnail, TrashedItem, UpdateCheck,
} from "./types";
import { thumbFromDto, thumbError } from "./thumbs";

export type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

// ---- Beta-kanal: oppdatering (M9). Eneste sted i frontend som berører oppdateringsplugin (se nonet.test.ts). ----
/** Det vi bruker av `Update` fra @tauri-apps/plugin-updater. */
export interface UpdateHandle {
  version: string;
  date?: string;
  body?: string;
  downloadAndInstall(onEvent: (e: { event: string; data?: { contentLength?: number; chunkLength?: number } }) => void): Promise<void>;
}
/** Tynt lag over pluginene, byttbart i tester. Pluginene lastes dynamisk: ingenting lastes før brukeren ber om det (eller auto er på). */
export interface UpdaterApi {
  /** Kjører vi i Tauri? Utenfor (nettleser/dev) finnes ingen oppdatering. */
  available(): boolean;
  check(): Promise<UpdateHandle | null>;
  relaunch(): Promise<void>;
  getVersion(): Promise<string>;
}
export const tauriUpdater: UpdaterApi = {
  available: () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window,
  check: async () => (await import("@tauri-apps/plugin-updater")).check(),
  relaunch: async () => (await import("@tauri-apps/plugin-process")).relaunch(),
  getVersion: async () => (await import("@tauri-apps/api/app")).getVersion(),
};

/** Åpner systemets mappevalg (tauri-plugin-dialog). Plugin lastes først ved bruk. null = avbrutt. */
export type PickDirectory = (title?: string) => Promise<string | null>;

const pickDirectory: PickDirectory = async (title) => {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === "string" ? r : null;
};

export class TauriBridge implements CoreBridge {
  readonly canPickFolder = true;
  /** Funnet oppdatering fra siste sjekk; `installUpdate` bruker den. */
  private pending: UpdateHandle | null = null;

  constructor(
    private readonly call: Invoke = invoke,
    private readonly updater: UpdaterApi = tauriUpdater,
    private readonly pick: PickDirectory = pickDirectory,
  ) {}

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
  listLibraryRoots(): Promise<LibraryRoot[]> {
    return this.call<LibraryRoot[]>("list_library_roots");
  }
  addLibraryRoot(path: string): Promise<LibraryRoot[]> {
    return this.call<LibraryRoot[]>("add_library_root", { path });
  }
  removeLibraryRoot(key: number): Promise<LibraryRoot[]> {
    return this.call<LibraryRoot[]>("remove_library_root", { key });
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

  // ---- Beta-kanal: oppdatering (M9) ----
  async getAppVersion(): Promise<string> {
    if (!this.updater.available()) return "dev";
    try {
      return await this.updater.getVersion();
    } catch {
      return "dev";
    }
  }
  async checkForUpdate(): Promise<UpdateCheck> {
    if (!this.updater.available()) {
      throw { code: "unsupported", detail: "Oppdatering virker bare i den installerte appen (ikke i nettleser eller utviklingsmodus)." };
    }
    let update: UpdateHandle | null;
    try {
      update = await this.updater.check();
    } catch (e) {
      throw { code: "update_check_failed", detail: String(e) };
    }
    this.pending = update;
    if (!update) return { available: false };
    return { available: true, version: update.version, notes: update.body ?? undefined, date: update.date ?? undefined };
  }
  async installUpdate(onProgress: (done: number, total: number | null) => void): Promise<void> {
    const update = this.pending;
    if (!update) throw { code: "no_update", detail: "Ingen oppdatering er funnet. Sjekk først." };
    let done = 0;
    let total: number | null = null;
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") {
          total = e.data?.contentLength ?? null;
          onProgress(0, total);
        } else if (e.event === "Progress") {
          done += e.data?.chunkLength ?? 0;
          onProgress(done, total);
        } else if (e.event === "Finished") {
          onProgress(total ?? done, total);
        }
      });
    } catch (e) {
      throw { code: "update_install_failed", detail: String(e) };
    }
    this.pending = null;
    await this.updater.relaunch();
  }
  getAutoUpdate(): Promise<boolean> {
    return this.call<boolean>("get_auto_update");
  }
  setAutoUpdate(on: boolean): Promise<boolean> {
    return this.call<boolean>("set_auto_update", { on });
  }
  getLanguage(): Promise<string | null> {
    return this.call<string | null>("get_language");
  }
  setLanguage(language: string): Promise<string> {
    return this.call<string>("set_language", { language });
  }

  // ---- Miniatyrer (M12) ----
  async getThumbnail(id: number): Promise<Thumbnail> {
    let dto: ThumbDto;
    try {
      dto = await this.call<ThumbDto>("get_thumbnail", { id });
    } catch (e) {
      throw thumbError(e);
    }
    return thumbFromDto(dto);
  }
}
