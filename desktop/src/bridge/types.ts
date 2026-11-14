// Typer som speiler fjell-shell::dto og dto_v1 (camelCase). Kjernen sender koder, aldri tekst.
// Rekkefølge og navn følger Rust-typene; endres de, endres begge sider (og tests_v1.rs i shell).
export type KindCode =
  | "exact_dup"
  | "visual_dup"
  | "blurry"
  | "screenshot"
  | "series"
  | "rule";

export interface KindSummary {
  kind: KindCode;
  count: number;
  bytes: number;
}

export interface AppState {
  backend: "mock" | "core";
  perKind: KindSummary[];
  reclaimableBytes: number;
  trashCount: number;
  trashBytes: number;
  trashExpired: number;
  freedBytes: number;
  scanPhase: string;
  scanDone: number;
  scanTotal: number;
  trashDays: number;
}

/** `code` er en stabil kode; ved `ack_required` er `detail` konsekvenskodene (kommaskilt, snake_case)
 *  som må bekreftes, og ved `core_error` er `detail` kjernens feilkode (f.eks. `empty_rule`). */
export interface BridgeError {
  code: string;
  detail: string;
}

// ---- V1 ----

/** Tall/ID bak et begrunnelsesmerke. */
export type Detail =
  | { kind: "none" }
  | { kind: "percent"; value: number }
  | { kind: "count"; value: number }
  | { kind: "position"; index: number; of: number }
  | { kind: "rule"; id: number }
  | { kind: "person"; id: number };

export interface Card {
  id: number;
  kind: KindCode;
  asset: number;
  group: number | null;
  keeper: number | null;
  /** Variantnavn i kjernen, f.eks. `SeriesMember`. */
  reason: string;
  detail: Detail;
  confidence: number;
  bytes: number;
  protectFlags: number;
}

export interface CardPage {
  cards: Card[];
  next: number | null;
}

export type Decision = "trash" | "keep";

export type ScopeCode = "asset" | "folder" | "album" | "person";

/** `target` er id (som tekst) eller mappeprefiks. */
export interface Scope {
  scope: ScopeCode;
  target: string;
}

export interface Protection {
  scope: Scope;
  createdAt: number;
}

/** Vilkår uten verdi har bare `kind`. Regler foreslår bare; ingenting slettes av dem. */
export type Condition =
  | { kind: "is_screenshot" }
  | { kind: "is_blurry" }
  | { kind: "is_exact_duplicate" }
  | { kind: "older_than_days"; value: number }
  | { kind: "larger_than_bytes"; value: number }
  | { kind: "smaller_than_bytes"; value: number }
  | { kind: "in_album"; value: number }
  | { kind: "in_folder"; value: string }
  | { kind: "mime"; value: string };

export interface RuleDraft {
  /** `null` oppretter en ny regel. */
  id: number | null;
  name: string;
  conditions: Condition[];
}

export interface Rule {
  id: number;
  name: string;
  conditions: Condition[];
  enabled: boolean;
  hits: number;
}

export interface RulePreview {
  hits: number;
  bytes: number;
  sample: number[];
}

export interface Person {
  id: number;
  name: string | null;
  faceCount: number;
  assetCount: number;
  cover: number | null;
  protected: boolean;
}

export interface KindStat {
  kind: KindCode;
  count: number;
  bytes: number;
}

export interface MonthStat {
  /** `ÅÅÅÅMM`, f.eks. 202611. */
  yearMonth: number;
  count: number;
  bytes: number;
}

export interface FolderStat {
  path: string;
  count: number;
  bytes: number;
}

export interface Dashboard {
  activeCount: number;
  activeBytes: number;
  trashedCount: number;
  trashedBytes: number;
  /** Kjernen påstår aldri frigjort plass før en purge er loggført. */
  freedBytes: number;
  perKind: KindStat[];
  byMonth: MonthStat[];
  topFolders: FolderStat[];
  protectedCount: number;
  seriesCount: number;
  peopleCount: number;
  rulesActive: number;
  /** Avgjorte av totale forslag (`dashboard.progress.label`). */
  progress: Progress;
  /** Foreslått neste steg som kode med tall (`dashboard.next.*`). Appen foreslår; ingenting skjer av seg selv. */
  nextStep: NextStep;
  /** «Tømt» per måned (UTC), eldst først. Ikke det samme som frigjort på disken. */
  freedByMonth: FreedMonth[];
}

export interface Progress {
  decided: number;
  total: number;
  permille: number;
}

/** `review`: gå gjennom `suggestionKind`; `empty_trash`: papirkurven kan tømmes; `nothing`. */
export interface NextStep {
  kind: "review" | "empty_trash" | "nothing";
  suggestionKind: string | null;
  count: number;
  bytes: number;
}

export interface FreedMonth {
  yearMonth: number;
  count: number;
  bytes: number;
}

export interface SeriesMember {
  asset: number;
  /** 1-basert; 1 = foreslått behold-kandidat. */
  rank: number;
  score: number;
  isKeeper: boolean;
  reason: string;
  detail: Detail;
  bytes: number;
  protectFlags: number;
}

export interface SeriesCompare {
  series: number;
  keeper: number;
  members: SeriesMember[];
}

export type SensitivityCode = "low" | "normal" | "high";

export interface Settings {
  trashDays: number;
  /** Nøkkel = forslagstype. */
  sensitivity: Record<KindCode, SensitivityCode>;
  /** Ansiktsindeks. Av som standard. */
  faceIndex: boolean;
}

export interface Thresholds {
  visualMaxHamming: number;
  blurMaxScore: number;
  seriesGapSecs: number;
  closedEyesMinConfidence: number;
}
