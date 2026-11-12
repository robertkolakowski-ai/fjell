// Hjem: ViewState + ren reduce (PLAN-UX-I18N del 3.2 og 4). partialAccess venter på rotstatus fra adapter (A2).
import type { AppState, BridgeError, KindCode, ScanPhase } from "../bridge/types";

export type HomeView =
  | { kind: "loading" }
  | { kind: "scanning"; summary: AppState; done: number; total: number }
  | { kind: "ready"; summary: AppState }
  | { kind: "allClean"; summary: AppState }
  | { kind: "error"; error: BridgeError };

export type HomeEvent =
  | { type: "loaded"; summary: AppState }
  | { type: "failed"; error: BridgeError }
  | { type: "retry" };

/** Skann pågår når fasen er aktiv. `idle` og `done` er hvile. */
export const isScanning = (p: ScanPhase): boolean => p === "listing" || p === "exact" || p === "exact_ready" || p === "analyzing";

export const pendingCount = (s: AppState): number => s.perKind.reduce((n, k) => n + k.count, 0);
export const kindOf = (s: AppState, kind: KindCode) => s.perKind.find((k) => k.kind === kind) ?? { kind, count: 0, bytes: 0 };

/** Andel av alt ryddbart (ventende + papirkurv) som ligger i ventende. Kun supplement til tallet i klartekst. */
export function ringFraction(s: AppState): number {
  const total = s.reclaimableBytes + s.trashBytes;
  return total > 0 ? s.reclaimableBytes / total : 0;
}

export const initialHome: HomeView = { kind: "loading" };

export function reduceHome(s: HomeView, e: HomeEvent): HomeView {
  switch (e.type) {
    case "loaded": {
      const sum = e.summary;
      if (isScanning(sum.scanPhase)) return { kind: "scanning", summary: sum, done: sum.scanDone, total: sum.scanTotal };
      // Aldri skannet (idle) er ikke «alt ryddet».
      if (sum.scanPhase === "done" && pendingCount(sum) === 0) return { kind: "allClean", summary: sum };
      return { kind: "ready", summary: sum };
    }
    case "failed":
      // Behold forrige data ved feil under polling.
      return s.kind === "loading" || s.kind === "error" ? { kind: "error", error: e.error } : s;
    case "retry":
      return s.kind === "error" ? { kind: "loading" } : s;
  }
}
