// MockBridge: deterministiske eksempeldata, samme grensesnitt. Forsinkelser kan slås av.
import type { CoreBridge } from "./core-bridge";
import {
  KIND_CODES, type AppState, type BridgeError, type Card, type CardPage, type Decision,
  type KindCode, type ReasonCode, type ScanPhase,
} from "./types";

export interface MockOptions {
  /** Simulert latens per kall i ms. 0 = av (tester). */
  delayMs?: number;
  trashDays?: number;
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
  private batches: number[][] = [];
  private nextBatch = 1;
  private phase: ScanPhase = "idle";
  private freed = 0;
  private readonly delayMs: number;
  private readonly days: number;

  constructor(opts: MockOptions = {}) {
    this.delayMs = opts.delayMs ?? 0;
    this.days = opts.trashDays ?? 30;
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
      scanTotal: this.phase === "idle" ? 0 : 100, trashDays: this.days,
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
    if (decision === "trash") {
      this.trashed.push(...(hit as Card[]));
      this.batches.push(ids);
    }
    return batch;
  }

  async undo(): Promise<number> {
    await this.wait();
    const ids = this.batches.pop();
    if (!ids) throw err("nothing_to_undo");
    const back = this.trashed.filter((c) => ids.includes(c.id));
    this.trashed = this.trashed.filter((c) => !ids.includes(c.id));
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
}
