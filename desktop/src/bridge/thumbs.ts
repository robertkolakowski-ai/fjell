// Miniatyrer i frontend: feilmapping, LRU-cache, deduplisering og kø (maks 4 samtidige). Ren logikk uten DOM.
// Bildedata kommer bare via IPC (`get_thumbnail`) og lever bare i minnet som data:-URL. Ingen nettverk, ingen blob:.
import type { CoreBridge } from "./core-bridge";
import { THUMB_ERROR_CODES, type BridgeError, type ThumbDto, type Thumbnail, type ThumbErrorCode } from "./types";

/** Ukjent eller manglende kode fra skallet gir `unavailable` (vi viser forklaring, aldri en tom rute). */
export function thumbError(e: unknown): BridgeError {
  if (e && typeof e === "object" && "code" in e) {
    const { code, detail } = e as { code: unknown; detail?: unknown };
    if (typeof code === "string" && (THUMB_ERROR_CODES as readonly string[]).includes(code)) {
      return { code, detail: typeof detail === "string" ? detail : "" };
    }
    return { code: "unavailable", detail: typeof detail === "string" ? detail : String(code) };
  }
  return { code: "unavailable", detail: typeof e === "string" ? e : "" };
}

const BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;

/** ThumbDto til data:-URL. Bare JPEG og ren base64 slippes gjennom (ingenting annet kan havne i src). */
export function thumbFromDto(dto: ThumbDto): Thumbnail {
  if (!dto || dto.mime !== "image/jpeg" || typeof dto.data !== "string" || !BASE64.test(dto.data)) {
    throw { code: "corrupt", detail: "ugyldig miniatyrsvar" } satisfies BridgeError;
  }
  return { url: `data:image/jpeg;base64,${dto.data}`, width: dto.width, height: dto.height };
}

export const CANCELLED = "cancelled";
export const isCancelled = (e: unknown): boolean => !!e && typeof e === "object" && (e as BridgeError).code === CANCELLED;

export interface ThumbHandle {
  promise: Promise<Thumbnail>;
  /** Slipp interessen. Er ingen andre igjen og hentingen ikke har startet, droppes den. Løftet avvises med `cancelled`. */
  cancel(): void;
}

interface Waiter { resolve(t: Thumbnail): void; reject(e: BridgeError): void }
interface Entry { id: number; waiters: Set<Waiter>; started: boolean }
type Cached = { ok: Thumbnail } | { err: BridgeError };

export interface ThumbStoreOptions {
  /** Antall oppføringer i LRU (også feil, så et bilde som ikke lar seg vise ikke spørres på nytt for hver rulling). */
  capacity?: number;
  concurrency?: number;
}

export class ThumbStore {
  private readonly cache = new Map<number, Cached>();
  private readonly entries = new Map<number, Entry>();
  private queue: number[] = [];
  private active = 0;
  private readonly capacity: number;
  private readonly concurrency: number;

  constructor(private readonly fetchOne: (id: number) => Promise<Thumbnail>, opts: ThumbStoreOptions = {}) {
    this.capacity = Math.max(1, opts.capacity ?? 200);
    this.concurrency = Math.max(1, opts.concurrency ?? 4);
  }

  get size(): number { return this.cache.size; }
  /** Antall hentinger som pågår nå (tester, og garanti for maks samtidighet). */
  get inFlight(): number { return this.active; }
  get queued(): number { return this.queue.length; }

  /** Ferdig svar i cache (og oppfrisker LRU), ellers undefined. */
  peek(id: number): Cached | undefined {
    const hit = this.cache.get(id);
    if (hit) { this.cache.delete(id); this.cache.set(id, hit); }
    return hit;
  }

  /** Forhåndshent uten å vente (kø som vanlig; ingen avbryting). */
  prefetch(id: number): void {
    if (this.cache.has(id)) return;
    void this.load(id).promise.catch(() => undefined);
  }

  /** Glem ett bilde (f.eks. etter gjenoppretting) eller alt. */
  invalidate(id?: number): void {
    if (id === undefined) this.cache.clear();
    else this.cache.delete(id);
  }

  load(id: number): ThumbHandle {
    const w = {} as Waiter;
    const promise = new Promise<Thumbnail>((resolve, reject) => { w.resolve = resolve; w.reject = reject; });
    promise.catch(() => undefined); // avbrutte løfter skal ikke gi «unhandled rejection» hos den som slapp dem
    const hit = this.peek(id);
    if (hit) {
      if ("ok" in hit) w.resolve(hit.ok); else w.reject(hit.err);
      return { promise, cancel: () => undefined };
    }
    let entry = this.entries.get(id);
    if (!entry) {
      entry = { id, waiters: new Set(), started: false };
      this.entries.set(id, entry);
      this.queue.push(id);
    }
    entry.waiters.add(w);
    queueMicrotask(() => this.pump());
    const e = entry;
    return {
      promise,
      cancel: () => {
        if (!e.waiters.delete(w)) return;
        w.reject({ code: CANCELLED, detail: "" });
        if (e.waiters.size === 0 && !e.started) {
          this.entries.delete(e.id);
          this.queue = this.queue.filter((q) => q !== e.id);
        }
      },
    };
  }

  private put(id: number, v: Cached): void {
    this.cache.delete(id);
    this.cache.set(id, v);
    while (this.cache.size > this.capacity) this.cache.delete(this.cache.keys().next().value as number);
  }

  private pump(): void {
    while (this.active < this.concurrency && this.queue.length > 0) {
      const id = this.queue.shift() as number;
      const entry = this.entries.get(id);
      if (!entry || entry.waiters.size === 0) { this.entries.delete(id); continue; }
      entry.started = true;
      this.active++;
      let p: Promise<Thumbnail>;
      try { p = this.fetchOne(id); } catch (e) { p = Promise.reject(e); }
      p.then(
        (t) => { this.put(id, { ok: t }); for (const w of entry.waiters) w.resolve(t); },
        (e) => { const err = thumbError(e); this.put(id, { err }); for (const w of entry.waiters) w.reject(err); },
      ).finally(() => {
        this.entries.delete(id);
        this.active--;
        this.pump();
      });
    }
  }
}

const stores = new WeakMap<CoreBridge, ThumbStore>();
/** Én butikk per bridge (delt av alle tiles). */
export function thumbsFor(bridge: CoreBridge): ThumbStore {
  let s = stores.get(bridge);
  if (!s) {
    s = new ThumbStore((id) => bridge.getThumbnail(id));
    stores.set(bridge, s);
  }
  return s;
}

export type { ThumbErrorCode };
