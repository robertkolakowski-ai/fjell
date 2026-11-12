// MockBridge: deterministiske eksempeldata, samme grensesnitt. Forsinkelser kan slås av.
import type { CoreBridge } from "./core-bridge";
import {
  KIND_CODES, type AppState, type BridgeError, type Card, type CardPage, type Decision,
  type EmptyResult, type KindCode, type PermanentAck, type ReasonCode, type ScanPhase,
} from "./types";

export interface MockOptions {
  /** Simulert latens per kall i ms. 0 = av (tester). */
  delayMs?: number;
  trashDays?: number;
  /** Simuler volum uten Papirkurv (nettverksstasjon): Papirkurv-modus gir `unsupported`. */
  noBin?: boolean;
}

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
  private readonly days: number;

  constructor(opts: MockOptions = {}) {
    this.delayMs = opts.delayMs ?? 0;
    this.days = opts.trashDays ?? 30;
    this.noBin = opts.noBin ?? false;
  }

  private wait(): Promise<void> {
    return this.delayMs > 0 ? new Promise((r) => setTimeout(r, this.delayMs)) : Promise.resolve();
  }

  async getState(): Promise<AppState> {
    await this.wait();
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
      trashExpired: 0, freedBytes: this.freed,
      scanPhase: this.phase, scanDone: this.phase === "done" ? 100 : 0,
      scanTotal: this.phase === "idle" ? 0 : 100, trashDays: this.days, freedSince: this.freedSince,
    };
  }

  async startScan(_full: boolean): Promise<void> {
    await this.wait();
    this.phase = "done";
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
    if (decision === "trash") this.trashed.push(...(hit as Card[]));
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
