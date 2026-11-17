// MockBridge: deterministiske eksempeldata, samme grensesnitt. Forsinkelser kan slås av.
import { cloudProvider } from "./cloud";
import type { CoreBridge } from "./core-bridge";
import {
  KIND_CODES, type AppState, type BridgeError, type Card, type CardPage, type Decision,
  type AppSettings, type EmptyResult, type KindCode, type LibraryRoot, type PermanentAck, type ReasonCode, type RestoreResult,
  type ScanPhase, type Thumbnail, type ThumbErrorCode, type TrashedItem, type UpdateCheck,
} from "./types";
import type {
  Condition, Dashboard, Person, Protection, Rule, RuleDraft, RulePreview, Scope, SensitivityCode, SeriesCompare, SeriesMember,
  SuggestionSettings, Thresholds,
} from "./types-v1";
import {
  cleanRuleName, folderMatches, MOCK_ALBUMS, MOCK_ASSETS, MOCK_BYTES, monthSeries, normalizeScope, PERSON_NAME_MAX, ruleCandidates, ruleHitBytes,
  SCOPE_FLAG, previewHasNoMatches, scopeKey, seedPeople, seedRules, thresholdsFor, TOP_FOLDERS, validateConditions, yearMonthOf, type MockPerson, type MockRule,
} from "./mock-v1";
import { MockRobust, type MockRobustOptions } from "./mock-robust";
import type { Backup, DiagReport, RebuildResult, IndexingSettings, IndexingStatus, IntegrityCode, RestoreReport } from "./types-robust";

export interface MockOptions {
  /** Simulert latens per kall i ms. 0 = av (tester). */
  delayMs?: number;
  trashDays?: number;
  /** Simuler volum uten Papirkurv (nettverksstasjon): Papirkurv-modus gir `unsupported`. */
  noBin?: boolean;
  /** Antall filer (de siste i «Nylig slettet») som Papirkurv ikke kan ta: lang sti uten 8.3-navn, UNC. Resten flyttes. */
  unsupportedCount?: number;
  /** Klokke (ms siden epoch). Tester styrer utløp med denne. */
  now?: () => number;
  /** Valgt mappe. Standard: en mappe er valgt. `null` = ingen (start av skann gir `no_library`). */
  library?: string | null;
  /** Første gang appen kjøres (onboarding vises). Standard: false. */
  firstRun?: boolean;
  /** Antall `getState`-kall skannet blir stående i «analyserer» før det er ferdig (tester av pause/fortsett/avbryt). Standard 0 = straks ferdig. */
  scanPolls?: number;
  /** Oppdatering (M9). none = ingen ny versjon, available = ny versjon, error = sjekk feiler (frakoblet), slow = ny versjon med treg installasjon (fremdrift synlig), fail = ny versjon der installasjonen feiler (reserveveien vises). Standard: none. */
  update?: MockUpdate;
  /** «Sjekk automatisk ved oppstart». Standard: false. */
  autoUpdate?: boolean;
  /** Mappevalg («Bla …»): en sti = valgt mappe, `null` = bruker avbryter, `undefined` (standard) = ikke tilgjengelig (som i nettleser). */
  pick?: string | null;
  /** Lagret språkvalg i «skallet» (system, nb, en). Standard null = aldri valgt (frontend migrerer localStorage). */
  language?: string | null;
  /** Miniatyrfeil per bilde-id (`asset`), f.eks. `{ 3: "corrupt" }`. Andre id-er gir et deterministisk SVG-bilde. */
  thumbErrors?: Record<number, ThumbErrorCode>;
  /** Simulert ventetid per miniatyr i ms. Standard: 40 når `delayMs` > 0, ellers 0 (e2e og ytelsesmåling). */
  thumbDelayMs?: number;
  /** Robusthet (docs/v1/ROBUSTHET-KONTRAKT.md): indeksens integritet (`ok` standard; alt annet gir trygg modus) og batteri. */
  integrity?: MockRobustOptions["integrity"];
  onBattery?: MockRobustOptions["onBattery"];
}

/** Deterministisk, fotolignende SVG (himmelgradient, sol, to fjellrygger) varianter på id. Hver femte er stående. */
export function mockThumbSvg(id: number): Thumbnail {
  let h = (Math.imul(id | 0, 2654435761) ^ 0x9e3779b9) >>> 0;
  const rnd = (): number => { h = (Math.imul(h ^ (h >>> 15), 2246822519) + 0x7f4a7c15) >>> 0; return h / 4294967296; };
  const portrait = id % 5 === 0;
  const w = portrait ? 192 : 256;
  const hgt = portrait ? 256 : 192;
  const hue = Math.floor(rnd() * 360);
  const sunX = Math.round(w * (0.15 + rnd() * 0.7));
  const sunY = Math.round(hgt * (0.18 + rnd() * 0.25));
  const ridge = (base: number, amp: number): string => {
    const pts = [`0,${hgt}`];
    const n = 5 + Math.floor(rnd() * 3);
    for (let i = 0; i <= n; i++) pts.push(`${Math.round((w * i) / n)},${Math.round(hgt * base - rnd() * amp * hgt)}`);
    pts.push(`${w},${hgt}`);
    return pts.join(" ");
  };
  const far = ridge(0.68, 0.3);
  const near = ridge(0.85, 0.22);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${hgt}" viewBox="0 0 ${w} ${hgt}">`
    + `<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="hsl(${hue},55%,38%)"/>`
    + `<stop offset="1" stop-color="hsl(${(hue + 40) % 360},70%,78%)"/></linearGradient></defs>`
    + `<rect width="${w}" height="${hgt}" fill="url(#s)"/><circle cx="${sunX}" cy="${sunY}" r="${Math.round(w * 0.07)}" fill="hsl(${(hue + 50) % 360},90%,90%)"/>`
    + `<polygon points="${far}" fill="hsl(${hue},25%,32%)"/><polygon points="${near}" fill="hsl(${hue},30%,16%)"/></svg>`;
  return { url: `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`, width: w, height: hgt };
}

export type MockUpdate = "none" | "available" | "error" | "slow" | "fail";
export const MOCK_UPDATES: readonly MockUpdate[] = ["none", "available", "error", "slow", "fail"];

const DAY = 86_400;
const QUARANTINE = ".fjell-karantene";

const REASON: Record<KindCode, ReasonCode> = {
  exact_dup: "ExactCopy", visual_dup: "VisualDuplicate", blurry: "Blurry", screenshot: "Screenshot", series: "SeriesMember", rule: "Rule",
};
/** Kort per serie (pluss behold-kandidaten, som er seriens første bilde). */
const SERIES_CARDS = 3;
const PER_KIND = 6;
const err = (code: string, detail = ""): BridgeError => ({ code, detail });

export function seedCards(): Card[] {
  const out: Card[] = [];
  let id = 1;
  KIND_CODES.forEach((kind, ki) => {
    for (let i = 0; i < PER_KIND; i++, id++) {
      const per = kind === "series" ? SERIES_CARDS : 2;
      const grouped = kind === "exact_dup" || kind === "visual_dup" || kind === "series";
      // Serie 42: første kort har lukkede øyne. Regel 1 eier kort 1-3 i regelkategorien, regel 2 de neste.
      const closed = kind === "series" && i === SERIES_CARDS;
      out.push({
        id, kind, asset: id,
        group: grouped ? ki * 10 + Math.floor(i / per) + 1 : null,
        keeper: grouped ? 1000 + id - (i % per) : null,
        reason: closed ? "ClosedEyes" : REASON[kind],
        detail: closed ? { kind: "count", value: 1 }
          : kind === "series" ? { kind: "position", index: 2 + (i % SERIES_CARDS), of: SERIES_CARDS + 1 }
          : kind === "rule" ? { kind: "rule", id: i < 3 ? 1 : 2 }
          : { kind: "none" },
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
  private readonly unsupportedCount: number;
  private readonly delayMs: number;
  private days: number;
  private readonly now: () => number;
  private library: string | null;
  /** Ekstra mapper (nøkkel ≥ 2). Mocken har ingen filer: kortene hører alle til første mappe. */
  private extraRoots: { key: number; path: string }[] = [];
  private nextRootKey = 2;
  private firstRun: boolean;
  private readonly scanPolls: number;
  private readonly update: MockUpdate;
  private auto: boolean;
  private language: string | null;
  private readonly thumbErrors: Record<number, ThumbErrorCode>;
  private readonly thumbDelayMs: number;
  // ---- V1-tilstand ----
  private protections: Protection[] = [];
  private rules: MockRule[] = seedRules();
  private nextRuleId = 3;
  /** Id-er for kort mocken lager selv (regeltreff); over alle seed-id-er. */
  private nextCardId = 5000;
  private people: MockPerson[] = seedPeople();
  private faceIndex = false;
  private sens = Object.fromEntries(KIND_CODES.map((k) => [k, "normal"])) as Record<KindCode, SensitivityCode>;
  /** Antall tømt fra «Nylig slettet» (teller som avgjort i fremdriften). */
  private purged = 0;
  private freedMonths = new Map<number, { count: number; bytes: number }>();
  private pollsLeft = 0;
  private paused = false;
  /** Sekunder siden epoch når bildet (`asset`) ble flyttet til «Nylig slettet». */
  private trashedAt = new Map<number, number>();
  /** Robusthet: kopier av indeksen, integritet, indekseringsinnstillinger (mock-robust.ts). */
  private readonly robust: MockRobust;

  readonly canPickFolder: boolean;
  private readonly pick: string | null;

  constructor(opts: MockOptions = {}) {
    this.canPickFolder = opts.pick !== undefined;
    this.pick = opts.pick ?? null;
    this.delayMs = opts.delayMs ?? 0;
    this.days = opts.trashDays ?? 30;
    this.noBin = opts.noBin ?? false;
    this.unsupportedCount = Math.max(0, Math.floor(opts.unsupportedCount ?? 0));
    this.now = opts.now ?? Date.now;
    this.library = opts.library === undefined ? "C:\\Bilder" : opts.library;
    this.firstRun = opts.firstRun ?? false;
    this.scanPolls = opts.scanPolls ?? 0;
    this.update = opts.update ?? "none";
    this.auto = opts.autoUpdate ?? false;
    this.language = opts.language ?? null;
    this.thumbErrors = opts.thumbErrors ?? {};
    this.thumbDelayMs = opts.thumbDelayMs ?? (this.delayMs > 0 ? 40 : 0);
    this.robust = new MockRobust(this.now, MOCK_ASSETS, { integrity: opts.integrity, onBattery: opts.onBattery });
  }


  private wait(): Promise<void> {
    return this.delayMs > 0 ? new Promise((r) => setTimeout(r, this.delayMs)) : Promise.resolve();
  }

  async getState(): Promise<AppState> {
    await this.wait();
    if (this.phase === "analyzing" && !this.paused && !this.robust.batteryPaused() && this.pollsLeft-- <= 0) this.phase = "done";
    const have = this.phase === "idle" ? [] : this.visible();
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
      cloudSync: this.cloud(),
      roots: this.roots(false),
    };
  }

  private roots(stats: boolean): LibraryRoot[] {
    if (this.library === null) return [];
    const all = [{ key: 1, path: this.library }, ...this.extraRoots];
    return all.map((r) => {
      const sep = r.path.includes("\\") ? "\\" : "/";
      const base: LibraryRoot = {
        key: r.key, path: r.path, primary: r.key === 1, cloudSync: cloudProvider(r.path),
        quarantineDir: `${r.path.replace(/[\\/]+$/, "")}${sep}${QUARANTINE}`,
      };
      if (!stats) return base;
      const own = r.key === 1;
      return {
        ...base, online: true,
        count: own ? this.cards.length + this.kept.length : 0,
        bytes: own ? [...this.cards, ...this.kept].reduce((s, c) => s + c.bytes, 0) : 0,
        videos: 0, videoBytes: 0, trashed: own ? this.trashed.length : 0,
      };
    });
  }

  async listLibraryRoots(): Promise<LibraryRoot[]> {
    await this.wait();
    if (this.library === null) throw err("no_library");
    return this.roots(true);
  }

  async addLibraryRoot(path: string): Promise<LibraryRoot[]> {
    await this.wait();
    if (this.library === null) throw err("no_library");
    const p = path.trim();
    if (p === "" || p.includes("\0")) throw err("not_a_folder", path);
    // Samme regel som skallet, på tekst: lik, inni eller rundt en eksisterende mappe.
    const norm = (s: string) => s.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase() + "/";
    const hit = this.roots(false).find((r) => norm(r.path).startsWith(norm(p)) || norm(p).startsWith(norm(r.path)));
    if (hit) throw err("root_overlap", `${p} overlapper ${hit.path}`);
    this.extraRoots.push({ key: this.nextRootKey++, path: p });
    return this.roots(true);
  }

  async removeLibraryRoot(key: number): Promise<LibraryRoot[]> {
    await this.wait();
    if (this.library === null) throw err("no_library");
    if (key === 1) throw err("root_is_primary");
    if (!this.extraRoots.some((r) => r.key === key)) throw err("not_found");
    this.extraRoots = this.extraRoots.filter((r) => r.key !== key);
    return this.roots(true);
  }

  /** Skysynket bibliotek etter mappenavn (samme regler som kjernen). */
  private cloud() {
    return this.library === null ? null : cloudProvider(this.library);
  }

  async startScan(_full: boolean): Promise<void> {
    await this.wait();
    if (this.library === null) throw err("no_library");
    this.paused = false;
    // Ansikter bygges bare ved skanning og bare når indeksen er på (etter en sletting bygges den opp igjen).
    if (this.faceIndex && this.people.length === 0) this.people = seedPeople();
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
    const all = this.visible().filter((c) => c.kind === kind && (cursor == null || c.id > cursor));
    const n = Math.min(Math.max(limit, 1), 200);
    const cards = all.slice(0, n);
    return { cards, next: all.length > n ? cards[cards.length - 1].id : null };
  }

  async decide(ids: number[], decision: Decision, confirmed: string[] = []): Promise<number> {
    await this.wait();
    const live = this.visible();
    const hit = ids.map((id) => live.find((c) => c.id === id) ?? null);
    if (hit.some((c) => c === null)) throw err("unknown_card");
    // Beskyttet av brukeren (bilde, mappe, album, person): papirkurv krever egen bekreftelse. Behold er alltid trygt.
    if (decision === "trash" && (hit as Card[]).some((c) => (c.protectFlags & SCOPE_FLAG) !== 0) && !confirmed.includes("protected_by_scope")) {
      throw err("ack_required", "protected_by_scope");
    }
    // Som skallet: papirkurv i skysynket bibliotek nektes FØR noe endres. Behold er tillatt.
    const cloud = decision === "trash" ? this.cloud() : null;
    if (cloud) throw err("cloud_synced_readonly", cloud);
    const batch = this.nextBatch++;
    // Lagre kortene slik de var (uten beskyttelsesflagget mocken legger på ved visning), så angre gir dem tilbake uendret.
    const picked = ids.map((id) => this.cards.find((c) => c.id === id) as Card);
    this.cards = this.cards.filter((c) => !ids.includes(c.id));
    if (decision === "trash") {
      this.trashed.push(...picked);
      for (const c of picked) this.trashedAt.set(c.asset, Math.floor(this.now() / 1000));
    }
    else this.kept.push(...picked);
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

  async getThumbnail(id: number): Promise<Thumbnail> {
    if (this.thumbDelayMs > 0) await new Promise((r) => setTimeout(r, this.thumbDelayMs));
    if (this.library === null) throw err("no_library");
    const code = this.thumbErrors[id];
    if (code) throw err(code, `mock: ${code}`);
    if (!Number.isInteger(id) || id <= 0) throw err("not_found");
    return mockThumbSvg(id);
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
    // Som skallet: `setLibrary` erstatter biblioteket med én mappe.
    this.extraRoots = [];
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

  async getLanguage(): Promise<string | null> {
    await this.wait();
    return this.language;
  }

  /** Som skallet: bare system, nb og en er gyldige («pseudo» avvises). */
  async setLanguage(language: string): Promise<string> {
    await this.wait();
    if (language !== "system" && language !== "nb" && language !== "en") throw err("bad_request", "language");
    this.language = language;
    return language;
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
    this.noteFreed(n, this.trashed.reduce((s, c) => s + c.bytes, 0));
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
      // Papirkurv-modus: filer Papirkurv ikke kan ta (`Unsupported`) blir liggende i karantenen, urørt. Aldri stille sletting.
      const stay = this.noBin ? n : Math.min(n, this.unsupportedCount);
      const moved = this.trashed.slice(0, n - stay);
      const ids = new Set(moved.map((c) => c.id));
      this.purged += moved.length;
      this.trashed = this.trashed.slice(n - stay);
      this.batches = this.batches.filter((b) => b.decision !== "trash" || !b.ids.every((i) => ids.has(i)));
      return { ...none, purged: moved.length, keptTrashed: stay, unsupported: stay };
    }
    // Permanent krever avkrysning og riktig antall (PermanentAck). Aldri uten.
    if (!permanent.checked || permanent.count !== n) throw err("ack_required", "permanent");
    this.trashed = [];
    this.batches = this.batches.filter((b) => b.decision !== "trash");
    this.noteFreed(n, bytes);
    this.freedSince ??= Date.now();
    return { ...none, purged: n, freedBytes: bytes };
  }

  // ---- Beta-kanal: oppdatering (M9) ----
  async getAppVersion(): Promise<string> {
    await this.wait();
    return "dev";
  }

  async checkForUpdate(): Promise<UpdateCheck> {
    await this.wait();
    if (this.update === "error") throw err("update_check_failed", "offline");
    if (this.update === "none") return { available: false };
    return {
      available: true, version: "0.1.99", date: "2026-11-12T10:00:00Z",
      notes: "Rettet en feil i Papirkurv.\nBedre ytelse ved skanning av store mapper.",
    };
  }

  /** Mock: en lokal sti uten skjema (ingen URL-er i bygget, se nonet.test.ts). Ekte adresse kommer fra skallet. */
  async getUpdateDownloadUrl(): Promise<string> {
    return "/fjell/releases/tag/beta";
  }

  async installUpdate(onProgress: (done: number, total: number | null) => void): Promise<void> {
    if (this.update === "fail") { onProgress(0, null); throw err("update_install_failed", "mock"); }
    if (this.update !== "available" && this.update !== "slow") throw err("no_update");
    const total = 24_000_000;
    const steps = this.update === "slow" ? 8 : 2;
    onProgress(0, total);
    for (let i = 1; i <= steps; i++) {
      if (this.update === "slow") await new Promise((r) => setTimeout(r, 400));
      onProgress(Math.round((total * i) / steps), total);
    }
  }

  async getAutoUpdate(): Promise<boolean> {
    await this.wait();
    return this.auto;
  }

  async setAutoUpdate(on: boolean): Promise<boolean> {
    await this.wait();
    this.auto = on;
    return this.auto;
  }

  // ---- V1 ----
  private noteFreed(count: number, bytes: number): void {
    this.purged += count;
    this.freed += bytes;
    const ym = yearMonthOf(this.now());
    const m = this.freedMonths.get(ym) ?? { count: 0, bytes: 0 };
    this.freedMonths.set(ym, { count: m.count + count, bytes: m.bytes + bytes });
  }

  /** Alle personer som ikke er slått inn i en annen, og bare når indeksen er på. */
  private livePeople(): MockPerson[] {
    return this.faceIndex ? this.people.filter((p) => p.mergedInto === null) : [];
  }
  private rootOf(p: MockPerson): MockPerson {
    let cur = p;
    for (let guard = 0; cur.mergedInto !== null && guard < 100; guard++) cur = this.people.find((x) => x.id === cur.mergedInto) ?? cur;
    return cur;
  }
  /** Personen og alle som er slått inn i den (direkte og indirekte). */
  private familyOf(id: number): MockPerson[] {
    const root = this.people.find((p) => p.id === id);
    return root ? this.people.filter((p) => this.rootOf(p).id === id) : [];
  }
  private personAssets(id: number): number[] {
    return [...new Set(this.familyOf(id).flatMap((p) => p.assets))];
  }
  private personProtected(id: number): boolean {
    return this.protections.some((p) => p.scope.scope === "person" && p.scope.target === String(id));
  }

  private inScope(asset: number, scope: Scope): boolean {
    switch (scope.scope) {
      case "asset": return scope.target === String(asset);
      case "folder": return folderMatches(this.library ?? "C:\\Bilder", asset, scope.target);
      case "album": return (MOCK_ALBUMS[Number(scope.target)] ?? []).includes(asset);
      case "person": return this.personAssets(Number(scope.target)).includes(asset);
    }
  }
  private isProtected(asset: number): boolean {
    return this.protections.some((p) => this.inScope(asset, p.scope));
  }
  private ruleLive(id: number): boolean {
    const r = this.rules.find((x) => x.id === id);
    return r !== undefined && r.enabled && !r.deleted;
  }

  /**
   * Forslagene slik kjernen viser dem: regelkort bare for aktive regler og aldri på beskyttede bilder; bilder brukeren
   * har beskyttet får `SCOPE_FLAG`, så skjermene aldri forhåndsvelger dem.
   */
  private visible(): Card[] {
    return this.cards
      .filter((c) => c.kind !== "rule" || (c.detail.kind === "rule" && this.ruleLive(c.detail.id) && !this.isProtected(c.asset)))
      .map((c) => (this.isProtected(c.asset) ? { ...c, protectFlags: c.protectFlags | SCOPE_FLAG } : c));
  }

  /** Regelens treff som kort: bilder som alt er avgjort foreslås ikke på nytt. */
  private syncRuleCards(rule: MockRule): void {
    const decided = new Set([...this.trashed, ...this.kept].map((c) => c.asset));
    this.cards = this.cards.filter((c) => !(c.kind === "rule" && c.detail.kind === "rule" && c.detail.id === rule.id));
    for (const asset of ruleCandidates(rule.conditions)) {
      if (decided.has(asset)) continue;
      this.cards.push({
        id: this.nextCardId++, kind: "rule", asset, group: null, keeper: null, reason: "Rule", detail: { kind: "rule", id: rule.id },
        confidence: 0.8, bytes: ruleHitBytes(asset), protectFlags: 0,
      });
    }
  }

  async protect(scope: Scope, on: boolean): Promise<void> {
    await this.wait();
    const s = normalizeScope(scope);
    const key = scopeKey(s);
    const has = this.protections.some((p) => scopeKey(p.scope) === key);
    if (!on) {
      this.protections = this.protections.filter((p) => scopeKey(p.scope) !== key);
      return;
    }
    const n = Number(s.target);
    const known = s.scope === "asset" ? Number.isInteger(n) && n >= 1 && n <= MOCK_ASSETS
      : s.scope === "album" ? n in MOCK_ALBUMS
      : s.scope === "person" ? this.livePeople().some((p) => p.id === n)
      : true;
    if (!known) throw err("unknown_target", `${s.scope}:${s.target}`);
    if (!has) this.protections.push({ scope: s, createdAt: Math.floor(this.now() / 1000) });
  }

  async listProtections(): Promise<Protection[]> {
    await this.wait();
    return this.protections.map((p) => ({ scope: { ...p.scope }, createdAt: p.createdAt }));
  }

  private ruleView(r: MockRule): Rule {
    const hits = r.enabled ? this.visible().filter((c) => c.kind === "rule" && c.detail.kind === "rule" && c.detail.id === r.id).length : 0;
    return { id: r.id, name: r.name, conditions: r.conditions.map((c) => ({ ...c })), enabled: r.enabled, hits };
  }

  async listRules(): Promise<Rule[]> {
    await this.wait();
    return this.rules.filter((r) => !r.deleted).map((r) => this.ruleView(r));
  }

  async saveRule(draft: RuleDraft): Promise<number> {
    await this.wait();
    validateConditions(draft.conditions);
    const name = cleanRuleName(draft.name);
    let rule: MockRule | undefined;
    if (draft.id === null) {
      rule = { id: this.nextRuleId++, name, conditions: draft.conditions.map((c) => ({ ...c })), enabled: true, deleted: false };
      this.rules.push(rule);
    } else {
      rule = this.rules.find((r) => r.id === draft.id && !r.deleted);
      if (!rule) throw err("rule_not_found");
      rule.name = name;
      rule.conditions = draft.conditions.map((c) => ({ ...c }));
    }
    this.syncRuleCards(rule);
    return rule.id;
  }

  async previewRule(conditions: Condition[]): Promise<RulePreview> {
    await this.wait();
    // Som kjernen: forhåndsvisning lagrer og validerer ingenting; tom regel gir null treff (se `previewHasNoMatches`).
    if (previewHasNoMatches(conditions)) return { hits: 0, bytes: 0, sample: [] };
    const decided = new Set([...this.trashed, ...this.kept].map((c) => c.asset));
    const hits = ruleCandidates(conditions).filter((a) => !decided.has(a) && !this.isProtected(a));
    return { hits: hits.length, bytes: hits.reduce((n, a) => n + ruleHitBytes(a), 0), sample: hits.slice(0, 3) };
  }

  private liveRule(id: number): MockRule {
    const r = this.rules.find((x) => x.id === id && !x.deleted);
    if (!r) throw err("rule_not_found");
    return r;
  }

  async toggleRule(id: number, enabled: boolean): Promise<void> {
    await this.wait();
    this.liveRule(id).enabled = enabled;
  }

  async deleteRule(id: number): Promise<void> {
    await this.wait();
    this.liveRule(id).deleted = true;
  }

  async restoreRule(id: number): Promise<void> {
    await this.wait();
    const r = this.rules.find((x) => x.id === id);
    if (!r) throw err("rule_not_found");
    r.deleted = false;
  }

  async listPeople(): Promise<Person[]> {
    await this.wait();
    return this.livePeople().map((p) => {
      const family = this.familyOf(p.id);
      const assets = this.personAssets(p.id);
      return {
        id: p.id, name: p.name, faceCount: family.reduce((n, f) => n + f.faces, 0), assetCount: assets.length,
        cover: assets[0] ?? null, protected: this.personProtected(p.id),
      };
    });
  }

  private livePerson(id: number): MockPerson {
    const p = this.livePeople().find((x) => x.id === id);
    if (!p) throw err("unknown_person");
    return p;
  }

  async namePerson(id: number, name: string | null): Promise<void> {
    await this.wait();
    const n = name === null ? null : name.trim();
    if (n !== null && [...n].length > PERSON_NAME_MAX) throw err("name_too_long");
    this.livePerson(id).name = n === "" ? null : n;
  }

  async mergePeople(target: number, absorbed: number): Promise<number> {
    await this.wait();
    const t = this.livePerson(target);
    const a = this.livePerson(absorbed);
    if (t.id === a.id) return t.id;
    a.mergedInto = t.id;
    return t.id;
  }

  async unmergePerson(absorbed: number): Promise<void> {
    await this.wait();
    const a = this.people.find((p) => p.id === absorbed);
    if (!a) throw err("unknown_person");
    if (a.mergedInto === null) throw err("not_merged");
    a.mergedInto = null;
  }

  async protectPerson(id: number, protect: boolean): Promise<void> {
    await this.wait();
    this.livePerson(id);
    return this.protect({ scope: "person", target: String(id) }, protect);
  }

  async getFaceIndex(): Promise<boolean> {
    await this.wait();
    return this.faceIndex;
  }

  async setFaceIndex(enabled: boolean): Promise<void> {
    await this.wait();
    this.faceIndex = enabled;
  }

  /** Fjerner ansiktsdata og personer, aldri bilder. Personbeskyttelser blir til beskyttelse av hvert bilde (ingen mister vernet). */
  async wipeFaceIndex(): Promise<number> {
    await this.wait();
    const faces = this.people.reduce((n, p) => n + p.faces, 0);
    const now = Math.floor(this.now() / 1000);
    const added: Protection[] = [];
    for (const prot of this.protections.filter((p) => p.scope.scope === "person")) {
      for (const asset of this.personAssets(Number(prot.scope.target))) {
        added.push({ scope: { scope: "asset", target: String(asset) }, createdAt: now });
      }
    }
    this.protections = this.protections.filter((p) => p.scope.scope !== "person");
    for (const p of added) if (!this.protections.some((x) => scopeKey(x.scope) === scopeKey(p.scope))) this.protections.push(p);
    this.people = [];
    return faces;
  }

  async getDashboard(): Promise<Dashboard> {
    await this.wait();
    const live = this.phase === "idle" ? [] : this.visible();
    const perKind = KIND_CODES.map((kind) => {
      const k = live.filter((c) => c.kind === kind);
      return { kind, count: k.length, bytes: k.reduce((n, c) => n + c.bytes, 0) };
    });
    const trashedBytes = this.trashed.reduce((n, c) => n + c.bytes, 0);
    const decided = this.kept.length + this.trashed.length + this.purged;
    const total = decided + live.length;
    const top = [...perKind].sort((a, b) => b.count - a.count || b.bytes - a.bytes)[0];
    return {
      activeCount: MOCK_ASSETS - this.trashed.length, activeBytes: MOCK_BYTES - trashedBytes,
      trashedCount: this.trashed.length, trashedBytes, freedBytes: this.freed,
      perKind, byMonth: monthSeries(this.now()), topFolders: TOP_FOLDERS(this.library ?? "C:\\Bilder"),
      protectedCount: this.protections.length,
      seriesCount: new Set(live.filter((c) => c.kind === "series").map((c) => c.group)).size,
      peopleCount: this.livePeople().length,
      rulesActive: this.rules.filter((r) => r.enabled && !r.deleted).length,
      progress: { decided, total, permille: total === 0 ? 0 : Math.round((1000 * decided) / total) },
      nextStep: top && top.count > 0
        ? { kind: "review", suggestionKind: top.kind, count: top.count, bytes: top.bytes }
        : this.trashed.length > 0
          ? { kind: "empty_trash", suggestionKind: null, count: this.trashed.length, bytes: trashedBytes }
          : { kind: "nothing", suggestionKind: null, count: 0, bytes: 0 },
      freedByMonth: [...this.freedMonths.entries()].sort((a, b) => a[0] - b[0]).map(([yearMonth, v]) => ({ yearMonth, ...v })),
    };
  }

  async compareSeries(series: number): Promise<SeriesCompare> {
    await this.wait();
    const cards = this.visible().filter((c) => c.kind === "series" && c.group === series);
    if (cards.length === 0 || cards[0].keeper === null) throw err("unknown_series");
    const keeper = cards[0].keeper;
    const flags = (asset: number): number => (this.isProtected(asset) ? SCOPE_FLAG : 0);
    // Behold-kandidaten (rang 1) forklares med øyne når et av kortene har lukkede øyne, ellers best samlet.
    const eyes = cards.some((c) => c.reason === "ClosedEyes");
    const members: SeriesMember[] = [{
      asset: keeper, rank: 1, score: 0.92, isKeeper: true, reason: eyes ? "KeeperEyesOpen" : "KeeperBestOverall", detail: { kind: "none" },
      bytes: cards[0].bytes + 60_000, protectFlags: flags(keeper),
    }];
    cards.forEach((c, i) => members.push({
      asset: c.asset, rank: i + 2, score: Math.round((0.84 - i * 0.07) * 100) / 100, isKeeper: false, reason: c.reason, detail: c.detail,
      bytes: c.bytes, protectFlags: c.protectFlags | flags(c.asset),
    }));
    return { series, keeper, members };
  }

  async getSuggestionSettings(): Promise<SuggestionSettings> {
    await this.wait();
    return { trashDays: this.days, sensitivity: { ...this.sens }, faceIndex: this.faceIndex };
  }

  async setSuggestionSettings(settings: SuggestionSettings): Promise<SuggestionSettings> {
    await this.wait();
    if (!Number.isInteger(settings.trashDays) || settings.trashDays < 1 || settings.trashDays > 365) throw err("bad_request", "trash_days");
    const next = { ...this.sens };
    for (const [k, v] of Object.entries(settings.sensitivity ?? {})) {
      if (!KIND_CODES.includes(k as KindCode) || (v !== "low" && v !== "normal" && v !== "high")) throw err("bad_request", "sensitivity");
      next[k as KindCode] = v;
    }
    this.days = settings.trashDays;
    this.sens = next;
    this.faceIndex = !!settings.faceIndex;
    return this.getSuggestionSettings();
  }

  async getThresholds(): Promise<Thresholds> {
    await this.wait();
    return thresholdsFor(this.sens);
  }

  // ---- Robusthet (docs/v1/ROBUSTHET-KONTRAKT.md); logikken ligger i mock-robust.ts ----

  private get scanning(): boolean {
    return this.phase === "analyzing";
  }

  async listBackups(): Promise<Backup[]> {
    await this.wait();
    return this.robust.list();
  }

  async createBackup(): Promise<Backup> {
    await this.wait();
    return this.robust.create();
  }

  async restoreBackup(id: number, acknowledged: boolean): Promise<RestoreReport> {
    await this.wait();
    return this.robust.restore(id, acknowledged, this.scanning);
  }

  async undoRestoreBackup(safety: number): Promise<RestoreReport> {
    await this.wait();
    return this.robust.undo(safety, this.scanning);
  }

  async checkIntegrity(): Promise<IntegrityCode> {
    await this.wait();
    return this.robust.integrity;
  }

  async rebuildIndex(): Promise<RebuildResult> {
    await this.wait();
    return this.robust.rebuild();
  }

  async getIndexingStatus(): Promise<IndexingStatus> {
    await this.wait();
    return this.robust.status(this.phase, this.paused);
  }

  async pauseIndexing(): Promise<void> {
    await this.wait();
    if (!this.scanning || this.paused) throw err("indexing_not_running");
    this.paused = true;
  }

  async resumeIndexing(): Promise<void> {
    await this.wait();
    if (!this.scanning || !this.paused) throw err("indexing_not_running");
    this.paused = false;
  }

  async cancelIndexing(): Promise<void> {
    await this.wait();
    if (!this.scanning) throw err("indexing_not_running");
    this.paused = false;
    this.phase = "done";
  }

  async getIndexingSettings(): Promise<IndexingSettings> {
    await this.wait();
    return { ...this.robust.settings };
  }

  async setIndexingSettings(settings: IndexingSettings): Promise<IndexingSettings> {
    await this.wait();
    return this.robust.setSettings(settings);
  }

  async previewDiagnostics(): Promise<DiagReport> {
    await this.wait();
    return this.diagnostics();
  }

  async exportDiagnostics(): Promise<DiagReport> {
    await this.wait();
    return this.diagnostics();
  }

  private diagnostics(): DiagReport {
    const live = this.phase === "idle" ? [] : this.visible();
    return this.robust.diag(
      {
        assetsActive: Math.max(0, MOCK_ASSETS - this.trashed.length), assetsTrashed: this.trashed.length,
        groups: new Set(live.map((c) => c.group).filter((g) => g !== null)).size, suggestions: live.length,
        rules: this.rules.filter((r) => !r.deleted).length, protections: this.protections.length,
      },
      this.robust.status(this.phase, this.paused).state,
    );
  }
}
