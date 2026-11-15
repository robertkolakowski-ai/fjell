// Robusthetstyper som speiler fjell-shell::dto_robust (camelCase): sikkerhetskopi av indeksen, integritet, indeksering og
// diagnose. Kjernen sender koder og tall, aldri tekst. Rekkefølge og navn følger Rust-typene; endres de, endres begge
// sider (og shell/tests/{backup,indexing,diag}.rs). Metodene ligger i CoreBridge (core-bridge.ts).
// Kontrakt og eierskap: docs/v1/ROBUSTHET-KONTRAKT.md.
import type { ScanPhase } from "./types";

/** Integritet av indeksen (`IntegrityStatus`). Alt annet enn `ok` gir trygg modus (RecoveryMode). */
export type IntegrityCode = "ok" | "degraded" | "corrupt" | "unreadable";
export const INTEGRITY_CODES: readonly IntegrityCode[] = ["ok", "degraded", "corrupt", "unreadable"];

export type BackupKind = "manual" | "auto" | "pre_restore";
export const BACKUP_KINDS: readonly BackupKind[] = ["manual", "auto", "pre_restore"];

/** Sikkerhetskopi av indeksen (bare metadata, aldri bilder). */
export interface Backup {
  id: number;
  kind: BackupKind;
  /** Unix-sekunder. */
  createdAt: number;
  bytes: number;
  /** Antall bilder i indeksen da kopien ble tatt. */
  assetCount: number;
  /** Kopiens egen integritet. En kopi som ikke er `ok` kan ikke gjenopprettes (`backup_corrupt`). */
  integrity: IntegrityCode;
}

/** Resultat av gjenoppretting. `safety` er kopien av indeksen slik den var rett før; `undoRestoreBackup(safety)` angrer. */
export interface RestoreReport {
  restored: number;
  safety: number;
  assetCount: number;
}

export type IndexingState = "idle" | "running" | "paused" | "paused_on_battery";
export const INDEXING_STATES: readonly IndexingState[] = ["idle", "running", "paused", "paused_on_battery"];

export interface IndexingStatus {
  state: IndexingState;
  /** Samme koder som `AppState.scanPhase`. */
  phase: ScanPhase;
  done: number;
  total: number;
  /** Bilder per sekund; 0 når ukjent. */
  ratePerSec: number;
  /** Gjenstående sekunder, eller null når det ikke kan anslås. */
  etaSecs: number | null;
}

export type IndexingPriority = "low" | "normal";
export const INDEXING_PRIORITIES: readonly IndexingPriority[] = ["low", "normal"];
/** Størst antall tråder (0 = automatisk). Speiler `MAX_INDEX_THREADS` i kjernen. */
export const MAX_INDEX_THREADS = 64;

export interface IndexingSettings {
  /** 0 = automatisk. */
  threads: number;
  priority: IndexingPriority;
  pauseOnBattery: boolean;
}

export type DiagMetricCode =
  | "schema_version" | "assets_active" | "assets_trashed" | "groups" | "suggestions" | "rules" | "protections" | "backups";
export const DIAG_METRIC_CODES: readonly DiagMetricCode[] = [
  "schema_version", "assets_active", "assets_trashed", "groups", "suggestions", "rules", "protections", "backups",
];

export interface DiagMetric {
  code: DiagMetricCode;
  value: number;
}

/** Diagnoserapport: tall, koder og en redigert tekstblokk (`kode=verdi` per linje). Ingen stier, filnavn eller bilder. */
export interface DiagReport {
  format: number;
  integrity: IntegrityCode;
  indexing: IndexingState;
  metrics: DiagMetric[];
  text: string;
  /** Hvor eksportfilen ble lagret (bare etter eksport, bare ekte modus). Stien vises som tekst; ingenting sendes. */
  savedPath?: string;
}

/** Resultat av «Bygg på nytt»: den ødelagte indeksen er lagt til side (ikke slettet), en ny bygges. */
export interface RebuildResult {
  corruptKept: boolean;
  scanning: boolean;
  /** «Nylig slettet» og angrehistorikk er nullstilt; filer fra karantenen er lagt tilbake, ingen er slettet. */
  trashStateReset: boolean;
}

/** Feilkodene skallet sender som `{ code: "core_error", detail: <kode> }` for denne flaten (speiler `ROBUST_ERROR_CODES`). */
export const ROBUST_ERROR_CODES: readonly string[] = [
  "backup_not_found", "backup_ack_required", "backup_corrupt", "backup_failed", "index_busy",
  "indexing_not_running", "indexing_invalid_settings", "diag_failed", "not_implemented",
];
