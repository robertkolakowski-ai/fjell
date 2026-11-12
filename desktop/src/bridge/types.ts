// Typer som speiler fjell-shell::dto (camelCase). Kjernen sender koder, aldri tekst.
export type KindCode = "exact_dup" | "visual_dup" | "blurry" | "screenshot";
export const KIND_CODES: readonly KindCode[] = ["exact_dup", "visual_dup", "blurry", "screenshot"];

/** Speiler fjell-core `ScanPhase` (phase_code i dto.rs). */
export type ScanPhase = "idle" | "listing" | "exact" | "exact_ready" | "analyzing" | "done";

/** Speiler fjell-core `ReasonCode` (variantnavn). Begrunnelsesmerke vises per forslag. */
export type ReasonCode =
  | "ExactCopy" | "VisualDuplicate" | "Blurry" | "Screenshot"
  | "HigherResolution" | "LessCompressed" | "Sharper" | "Older"
  | "ShotPlatformFlag" | "ShotScreenSize" | "ShotNoCameraExif" | "ShotPng";

export interface KindSummary {
  kind: KindCode;
  count: number;
  bytes: number;
}

export interface ScanStatus {
  phase: ScanPhase;
  done: number;
  total: number;
}

/** «Nylig slettet». */
export interface TrashSummary {
  count: number;
  bytes: number;
  expired: number;
  freedBytes: number;
  days: number;
}

export interface AppState {
  backend: "mock" | "core";
  perKind: KindSummary[];
  reclaimableBytes: number;
  trashCount: number;
  trashBytes: number;
  trashExpired: number;
  freedBytes: number;
  scanPhase: ScanPhase;
  scanDone: number;
  scanTotal: number;
  trashDays: number;
  /** Tidspunkt (ms siden epoch) for første permanente sletting, til historikklinjen. Kjernen leverer det ikke ennå. */
  freedSince?: number | null;
}

/** Brukerens ekstra bekreftelse for permanent sletting (avkrysning + antallet på knappen). Speiler PermanentAckDto. */
export interface PermanentAck {
  checked: boolean;
  count: number;
}

/** Resultat av «Tøm». Alt som ikke er `purged` ligger fortsatt i «Nylig slettet». Speiler EmptyDto. */
export interface EmptyResult {
  purged: number;
  keptTrashed: number;
  /** Volum uten Papirkurv: ikke slettet. UI kan tilby permanent med egen advarsel. */
  unsupported: number;
  cancelled: number;
  unavailable: number;
  freedBytes: number;
}

/** Ett forslag (kort). `group`/`keeper` er satt for dubletter. */
export interface Card {
  id: number;
  kind: KindCode;
  asset: number;
  group: number | null;
  keeper: number | null;
  reason: ReasonCode;
  confidence: number;
  bytes: number;
  protectFlags: number;
}

export interface CardPage {
  cards: Card[];
  next: number | null;
}

/** Gruppe avledet av kort med samme `group` (rent, se groups.ts). */
export interface Group {
  id: number;
  keeper: number | null;
  cards: Card[];
  bytes: number;
}

export type Decision = "trash" | "keep";

export type BridgeErrorCode =
  | "ack_required" | "unknown_card" | "unknown_asset" | "nothing_to_undo" | "not_undoable"
  | "already_purged" | "purge_stale" | "plan_stale" | "not_trashed" | "core_error"
  | "bad_request" | "unsupported";

export interface BridgeError {
  code: BridgeErrorCode | string;
  detail: string;
}
