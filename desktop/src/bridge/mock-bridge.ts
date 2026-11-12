// MockBridge: deterministiske eksempeldata, samme grensesnitt. Forsinkelser kan slås av.
import type { CoreBridge } from "./core-bridge";
import {
  KIND_CODES, type AppState, type BridgeError, type Card, type CardPage, type Decision,
  type AppSettings, type EmptyResult, type KindCode, type PermanentAck, type ReasonCode, type RestoreResult,
  type ScanPhase, type TrashedItem,
} from "./types";

export interface MockOptions {
  /** Simulert latens per kall i ms. 0 = av (tester). */
  delayMs?: number;
  trashDays?: number;
  /** Simuler volum uten Papirkurv (nettverksstasjon): Papirkurv-modus gir `unsupported`. */
  noBin?: boolean;
  /** Klokke (ms siden epoch). Tester styrer utløp med denne. */
  now?: () => number;
  /** Valgt mappe. Standard: en mappe er valgt. `null` = ingen (start av skann gir `no_library`). */
  library?: string | null;
  /** Første gang appen kjøres (onboarding vises). Standard: false. */
  firstRun?: boolean;
  /** Antall `getState`-kall skannet blir stående i «analyserer» før det er ferdig (tester av pause/fortsett/avbryt). Standard 0 = straks ferdig. */
  scanPolls?: number;
  /** Mappevalg («Bla …»): en sti = valgt mappe, `null` = bruker avbryter, `undefined` (standard) = ikke tilgjengelig (som i nettleser). */
  pick?: string | null;
}

const DAY = 86_400;
const QUARANTINE = ".fjell-karantene";

const REASON: Record<KindCode, ReasonCode> = {
  exact_dup: "ExactCopy", visual_dup: "VisualDuplicate", blurry: "Blurry", screenshot: "Screenshot",
};
const PER_KIND = 6;
const err = (code: string, detail = ""): BridgeError => ({ code, detail });

export function seedCards(): Card[] {
  const out: Card[] = [];
  let id = 1;
  KIND_CODES.forEach((kind, ki) => {
    for (let i = 0; i < PER_KIND; i++, id++) {
      const grouped = kind === "exact_dup" || kind === "visual_dup";
      out.push({
        id, kind, asset: id,
        group: grouped ? ki * 10 + Math.floor(i / 2) + 1 : null,
        keeper: grouped ? 1000 + id - (i % 2) : null,
        reason: REASON[kind],
        confidence: kind === "exact_dup" ? 1 : 0.8 + (i % 3) / 20,
        bytes: 1_000_000 * (ki + 1) + i * 1000,
        protectFlags: 0,
      });
    }
  });
  return out;
}

export class MockBridge implements CoreBridge {
  private cards = seedCards();
  private trashed: Card[] = [];
  /** Angre-stabel: både papirkurv og behold er batcher (kjernen gir batch-id for begge). */
  private batches: { ids: number[]; decision: Decision }[] = [];
  private kept: Card[] = [];
  private nextBatch = 1;
  private phase: ScanPhase = "idle";
  private freed = 0;
  private freedSince: number | null = null;
  private readonly noBin: boolean;
  private readonly delayMs: number;
  private days: number;
  private readonly now: () => number;
  private library: string | null;
  private firstRun: boolean;
  private readonly scanPolls: number;
  private pollsLeft = 0;
  private paused = false;
  /** Sekunder siden epoch når bildet (`asset`) ble flyttet til «Nylig slettet». */
  private trashedAt = new Map<number, number>();

  readonly canPickFolder: boolean;
  private readonly pick: string | null;

  constructor(opts: MockOptions = {}) {
    this.canPickFolder = opts.pick !== undefined;
    this.pick = opts.pick ?? null;
    this.delayMs = opts.delayMs ?? 0;
    this.days = opts.trashDays ?? 30;
    this.noBin = opts.noBin ?? false;
    this.now = opts.now ?? Date.now;
    this.library = opts.library === undefined ? "C:\\Bilder" : opts.library;
    this.firstRun = opts.firstRun ?? false;
    this.scanPolls = opts.scanPolls ?? 0;
  }


  private wait(): Promise<void> {
    return this.delayMs > 0 ? new Promise((r) => setTimeout(r, this.delayMs)) : Promise.resolve();
  }

  async getState(): Promise<AppState> {
    await this.wait();
    if (this.phase === "analyzing" && !this.paused && this.pollsLeft-- <= 0) this.phase = "done";
    const have = this.phase === "idle" ? [] : this.cards;
    const perKind = KIND_CODES.map((kind) => {
      const k = have.filter((c) => c.kind === kind);
      return { kind, count: k.length, bytes: k.reduce((s, c) => s + c.bytes, 0) };
    });
    return {
      backend: "mock", perKind,
      reclaimableBytes: perKind.reduce((s, k) => s + k.bytes, 0),
      trashCount: this.trashed.length,
      trashBytes: this.trashed.reduce((s, c) => s + c.bytes, 0),
      trashExpired: this.trashedItems().filter((i) => i.expired).length, freedBytes: this.freed,
      scanPhase: this.phase, scanDone: this.phase === "done" ? 100 : this.phase === "idle" ? 0 : 40,
      scanTotal: this.phase === "idle" ? 0 : 100, trashDays: this.days, freedSince: this.freedSince,
      scanPaused: this.paused,
    };
  }

  async startScan(_full: boolean): Promise<void> {
    await this.wait();
    if (this.library === null) throw err("no_library");
    this.paused = false;
    this.begin();
  }

  private begin(): void {
    this.pollsLeft = this.scanPolls;
    this.phase = this.scanPolls > 0 ? "analyzing" : "done";
  }

  async pauseScan(): Promise<void> {
    await this.wait();
    if (this.phase === "analyzing") this.paused = true;
  }

  async resumeScan(): Promise<void> {
    await this.wait();
    if (this.paused) {
      this.paused = false;
      this.phase = "analyzing";
    }
  }

  async cancelScan(): Promise<void> {
    await this.wait();
    this.paused = false;
    // Det som er funnet beholdes: skannet regnes som ferdig for det som er analysert.
    if (this.phase === "analyzing") this.phase = "done";
  }

  async listCards(kind: KindCode, limit: number, cursor: number | null = null): Promise<CardPage> {
    await this.wait();
    if (!KIND_CODES.includes(kind)) throw err("bad_request", "unknown kind");
    const all = this.cards.filter((c) => c.kind === kind && (cursor == null || c.id > cursor));
    const n = Math.min(Math.max(limit, 1), 200);
    const cards = all.slice(0, n);
    return { cards, next: all.length > n ? cards[cards.length - 1].id : null };
  }

  async decide(ids: number[], decision: Decision): Promise<number> {
    await this.wait();
    const hit = ids.map((id) => this.cards.find((c) => c.id === id) ?? null);
    if (hit.some((c) => c === null)) throw err("unknown_card");
    const batch = this.nextBatch++;
    this.cards = this.cards.filter((c) => !ids.includes(c.id));
    if (decision === "trash") {
      this.trashed.push(...(hit as Card[]));
      for (const c of hit as Card[]) this.trashedAt.set(c.asset, Math.floor(this.now() / 1000));
    }
    else this.kept.push(...(hit as Card[]));
    this.batches.push({ ids, decision });
    return batch;
  }

  async setKeeper(group: number, asset: number): Promise<void> {
    await this.wait();
    const members = this.cards.filter((c) => c.group === group);
    if (members.length === 0) throw err("unknown_card", "unknown group");
    const old = members[0].keeper;
    if (asset === old) return;
    const hit = members.find((c) => c.asset === asset);
    if (!hit || old === null) throw err("unknown_asset");
    // Byt plass: forslaget får den gamle keeperens asset, alle får ny keeper.
    this.cards = this.cards.map((c) =>
      c.group !== group ? c : { ...c, keeper: asset, asset: c.id === hit.id ? old : c.asset },
    );
  }

  async undo(): Promise<number> {
    await this.wait();
    const last = this.batches.pop();
    if (!last) throw err("nothing_to_undo");
    const { ids, decision } = last;
    const pool = decision === "trash" ? this.trashed : this.kept;
    const back = pool.filter((c) => ids.includes(c.id));
    if (decision === "trash") this.trashed = this.trashed.filter((c) => !ids.includes(c.id));
    else this.kept = this.kept.filter((c) => !ids.includes(c.id));
    this.cards = [...this.cards, ...back].sort((a, b) => a.id - b.id);
    return this.nextBatch++;
  }

  private trashedItems(): TrashedItem[] {
    const nowS = Math.floor(this.now() / 1000);
    return this.trashed
      .map((c) => {
        const trashedAt = this.trashedAt.get(c.asset) ?? nowS;
        const expiresAt = trashedAt + this.days * DAY;
        return { asset: c.asset, name: `IMG_${String(c.asset).padStart(4, "0")}.jpg`, bytes: c.bytes, trashedAt, expiresAt, expired: expiresAt <= nowS };
      })
      .sort((a, b) => a.trashedAt - b.trashedAt || a.asset - b.asset);
  }

  async listTrashed(): Promise<TrashedItem[]> {
    await this.wait();
    return this.trashedItems();
  }

  async restore(assets: number[]): Promise<RestoreResult> {
    await this.wait();
    const back = this.trashed.filter((c) => assets.includes(c.asset));
    if (back.length === 0) return { restored: 0 };
    const ids = new Set(back.map((c) => c.id));
    this.trashed = this.trashed.filter((c) => !ids.has(c.id));
    back.forEach((c) => this.trashedAt.delete(c.asset));
    // Gjenopprettede bilder foreslås ikke på nytt (som i kjernen), så de legges ikke tilbake som kort.
    // Angre-stabelen mister dem: en batch uten gjenværende bilder kan ikke angres.
    this.batches = this.batches
      .map((b) => ({ ...b, ids: b.decision === "trash" ? b.ids.filter((i) => !ids.has(i)) : b.ids }))
      .filter((b) => b.ids.length > 0);
    return { restored: back.length };
  }

  async restoreAll(): Promise<RestoreResult> {
    await this.wait();
    return this.restore(this.trashed.map((c) => c.asset));
  }

  async getLibrary(): Promise<string | null> {
    await this.wait();
    return this.library;
  }

  async pickFolder(_title?: string): Promise<string | null> {
    await this.wait();
    if (!this.canPickFolder) throw err("unsupported", "pickFolder");
    return this.pick;
  }

  async setLibrary(path: string): Promise<void> {
    await this.wait();
    if (path.trim() === "" || path.includes("\0")) throw err("not_a_folder", path);
    this.library = path.trim();
    this.firstRun = false;
  }

  async getFirstRun(): Promise<boolean> {
    await this.wait();
    return this.firstRun && this.library === null;
  }

  async completeOnboarding(): Promise<void> {
    await this.wait();
    this.firstRun = false;
  }

  async getSettings(): Promise<AppSettings> {
    await this.wait();
    return this.settings();
  }

  async setTrashDays(days: number): Promise<AppSettings> {
    await this.wait();
    if (!Number.isInteger(days) || days < 1 || days > 365) throw err("bad_request", "trash_days");
    this.days = days;
    return this.settings();
  }

  private settings(): AppSettings {
    const sep = this.library?.includes("\\") ? "\\" : "/";
    return {
      trashDays: this.days, emptyDefault: "recycle",
      quarantineDir: this.library === null ? null : `${this.library.replace(/[\\/]+$/, "")}${sep}${QUARANTINE}`,
    };
  }

  async purge(): Promise<number> {
    await this.wait();
    const n = this.trashed.length;
    this.freed += this.trashed.reduce((s, c) => s + c.bytes, 0);
    this.trashed = [];
    this.batches = [];
    return n;
  }

  async emptyTrash(permanent: PermanentAck | null = null): Promise<EmptyResult> {
    await this.wait();
    const n = this.trashed.length;
    const bytes = this.trashed.reduce((sum, c) => sum + c.bytes, 0);
    const none: EmptyResult = { purged: 0, keptTrashed: 0, unsupported: 0, cancelled: 0, unavailable: 0, freedBytes: 0 };
    if (permanent === null) {
      // Papirkurv-modus: filene forlater karantenen, men plassen er ikke frigjort ennå.
      if (this.noBin) return { ...none, unsupported: n, keptTrashed: n };
      this.trashed = [];
      this.batches = this.batches.filter((b) => b.decision !== "trash");
      return { ...none, purged: n };
    }
    // Permanent krever avkrysning og riktig antall (PermanentAck). Aldri uten.
    if (!permanent.checked || permanent.count !== n) throw err("ack_required", "permanent");
    this.trashed = [];
    this.batches = this.batches.filter((b) => b.decision !== "trash");
    this.freed += bytes;
    this.freedSince ??= Date.now();
    return { ...none, purged: n, freedBytes: bytes };
  }
}
