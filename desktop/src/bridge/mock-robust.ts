// Robusthet i MockBridge: sikkerhetskopi av indeksen, integritet, indeksering og diagnose. Deterministisk (klokke fra kalleren).
// Semantikken følger kontrakten (docs/v1/ROBUSTHET-KONTRAKT.md): kopier inneholder bare indeksen; gjenoppretting krever
// avkrysning, tar først en `pre_restore`-kopi og kan angres; ødelagte kopier kan ikke gjenopprettes; diagnosen har bare
// koder og tall. Feilkodene er de samme som kjernen sender (`ROBUST_ERROR_CODES`).
import type { BridgeError, ScanPhase } from "./types";
import type {
  Backup, DiagMetric, DiagReport, IndexingSettings, IndexingState, IndexingStatus, IntegrityCode, RestoreReport,
} from "./types-robust";
import { INDEXING_PRIORITIES, MAX_INDEX_THREADS } from "./types-robust";

const err = (code: string, detail = ""): BridgeError => ({ code, detail });

/** Sekunder i et døgn. */
const DAY = 86_400;

/** Rapportens tekstblokk, lik `DiagReport::render` i kjernen: `kode=verdi` per linje. */
export function renderDiag(r: Omit<DiagReport, "text">): string {
  const lines = [`format=${r.format}`, `integrity=${r.integrity}`, `indexing=${r.indexing}`, ...r.metrics.map((m) => `${m.code}=${m.value}`)];
  return lines.join("\n") + "\n";
}

export interface MockRobustOptions {
  /** Gjeldende indeks: `ok` (standard) eller en tilstand som gir trygg modus. */
  integrity?: IntegrityCode;
  /** PC-en går på batteri (indekseringen pauses når «pause på batteri» er på). Standard: false. */
  onBattery?: boolean;
}

export class MockRobust {
  integrity: IntegrityCode;
  onBattery: boolean;
  settings: IndexingSettings = { threads: 0, priority: "low", pauseOnBattery: true };
  private backups: Backup[];
  private nextId = 3;

  constructor(private readonly now: () => number, private readonly assets: number, opts: MockRobustOptions = {}) {
    this.integrity = opts.integrity ?? "ok";
    this.onBattery = opts.onBattery ?? false;
    const t = Math.floor(now() / 1000);
    this.backups = [
      { id: 1, kind: "auto", createdAt: t - 7 * DAY, bytes: this.sizeOf(assets), assetCount: assets, integrity: "ok" },
      { id: 2, kind: "manual", createdAt: t - DAY, bytes: this.sizeOf(assets), assetCount: assets, integrity: "ok" },
    ];
  }

  private sizeOf(assets: number): number {
    return 65_536 + assets * 480;
  }

  private add(kind: Backup["kind"]): Backup {
    const b: Backup = {
      id: this.nextId++, kind, createdAt: Math.floor(this.now() / 1000), bytes: this.sizeOf(this.assets),
      assetCount: this.assets, integrity: this.integrity,
    };
    this.backups.push(b);
    return b;
  }

  list(): Backup[] {
    return [...this.backups].sort((a, b) => b.createdAt - a.createdAt || b.id - a.id).map((b) => ({ ...b }));
  }

  create(): Backup {
    // En ødelagt indeks kopieres ikke som en vanlig kopi (den ville bare lage en ubrukelig kopi).
    if (this.integrity !== "ok") throw err("backup_failed", this.integrity);
    return { ...this.add("manual") };
  }

  /** `scanning`: skanning pågår, og indeksen kan ikke byttes midt i (`index_busy`). */
  restore(id: number, acknowledged: boolean, scanning: boolean): RestoreReport {
    if (!acknowledged) throw err("backup_ack_required");
    const b = this.backups.find((x) => x.id === id);
    if (!b) throw err("backup_not_found");
    if (b.integrity !== "ok") throw err("backup_corrupt");
    if (scanning) throw err("index_busy");
    const safety = this.add("pre_restore");
    this.integrity = "ok";
    return { restored: b.id, safety: safety.id, assetCount: b.assetCount };
  }

  undo(safety: number, scanning: boolean): RestoreReport {
    const b = this.backups.find((x) => x.id === safety && x.kind === "pre_restore");
    if (!b) throw err("backup_not_found");
    if (scanning) throw err("index_busy");
    // Indeksen er som den var rett før gjenopprettingen (også når den da var ødelagt).
    this.integrity = b.integrity;
    return { restored: b.id, safety: b.id, assetCount: b.assetCount };
  }

  setSettings(s: IndexingSettings): IndexingSettings {
    const ok = Number.isInteger(s?.threads) && s.threads >= 0 && s.threads <= MAX_INDEX_THREADS
      && INDEXING_PRIORITIES.includes(s.priority) && typeof s.pauseOnBattery === "boolean";
    if (!ok) throw err("indexing_invalid_settings");
    this.settings = { threads: s.threads, priority: s.priority, pauseOnBattery: s.pauseOnBattery };
    return { ...this.settings };
  }

  /** Pauset av batteri (bare mens indeksering pågår). */
  batteryPaused(): boolean {
    return this.onBattery && this.settings.pauseOnBattery;
  }

  status(phase: ScanPhase, paused: boolean): IndexingStatus {
    const active = phase === "listing" || phase === "exact" || phase === "analyzing";
    if (!active) return { state: "idle", phase, done: 0, total: 0, ratePerSec: 0, etaSecs: null };
    const state: IndexingState = paused ? "paused" : this.batteryPaused() ? "paused_on_battery" : "running";
    const total = this.assets;
    const done = Math.round(total * 0.4);
    const rate = state === "running" ? 120 : 0;
    return { state, phase, done, total, ratePerSec: rate, etaSecs: rate > 0 ? Math.ceil((total - done) / rate) : null };
  }

  diag(counts: { assetsActive: number; assetsTrashed: number; groups: number; suggestions: number; rules: number; protections: number }, indexing: IndexingState): DiagReport {
    const metrics: DiagMetric[] = [
      { code: "schema_version", value: 5 },
      { code: "assets_active", value: counts.assetsActive },
      { code: "assets_trashed", value: counts.assetsTrashed },
      { code: "groups", value: counts.groups },
      { code: "suggestions", value: counts.suggestions },
      { code: "rules", value: counts.rules },
      { code: "protections", value: counts.protections },
      { code: "backups", value: this.backups.length },
    ];
    const base = { format: 1, integrity: this.integrity, indexing, metrics };
    return { ...base, text: renderDiag(base) };
  }
}
