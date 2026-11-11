// Typer som speiler fjell-shell::dto (camelCase). Kjernen sender koder, aldri tekst.
export type KindCode = "exact_dup" | "visual_dup" | "blurry" | "screenshot";

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

export interface BridgeError {
  code: string;
  detail: string;
}
