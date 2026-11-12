// Papirkurv («Nylig slettet», PLAN-UX-I18N 3.6 og 4): ViewState + ren reduce. Ingen I/O.
// Tøm: appens ark med valg Windows Papirkurv (standard) eller permanent. Permanent har eget trinn med avkrysning
// (PermanentAck: checked + riktig antall). Volum uten Papirkurv: bare permanent, aldri automatisk.
import type { AppState, BridgeError, EmptyResult, PermanentAck } from "../bridge/types";

export type PurgeMode = "recycle" | "permanent";
export type TrashPhase = "loading" | "empty" | "list" | "confirmPurge" | "confirmPermanent" | "purging" | "purged" | "error";

export interface TrashInfo {
  count: number;
  bytes: number;
  expired: number;
  freedBytes: number;
  freedSince: number | null;
  days: number;
}

export interface PurgeOutcome {
  mode: PurgeMode;
  count: number;
  /** Elementer som ikke kunne flyttes og ligger fortsatt i «Nylig slettet». */
  marked: number;
  freedBytes: number;
}

export interface TrashState {
  phase: TrashPhase;
  info: TrashInfo | null;
  mode: PurgeMode;
  ack: boolean;
  /** Volumet har ikke Papirkurv: bare permanent tilbys. */
  noBin: boolean;
  outcome: PurgeOutcome | null;
  error: BridgeError | null;
}

export type TrashEvent =
  | { type: "loaded"; state: AppState }
  | { type: "failed"; error: BridgeError }
  | { type: "openPurge" }
  | { type: "setMode"; mode: PurgeMode }
  | { type: "continue" }
  | { type: "setAck"; value: boolean }
  | { type: "cancel" }
  /** Slett permanent trykket. Krever avkrysning. */
  | { type: "confirmPermanent" }
  | { type: "purged"; mode: PurgeMode; result: EmptyResult }
  | { type: "purgeFailed"; error: BridgeError }
  | { type: "dismiss" }
  | { type: "reload" };

export const initialTrash: TrashState = { phase: "loading", info: null, mode: "recycle", ack: false, noBin: false, outcome: null, error: null };

export const infoOf = (s: AppState): TrashInfo => ({
  count: s.trashCount, bytes: s.trashBytes, expired: s.trashExpired, freedBytes: s.freedBytes,
  freedSince: s.freedSince ?? null, days: s.trashDays,
});

/** Parametere til bridge.emptyTrash for valgt modus. Permanent sender alltid avkrysning og antall. */
export const ackFor = (s: TrashState): PermanentAck | null =>
  s.mode === "permanent" && s.info ? { checked: s.ack, count: s.info.count } : null;

const listOf = (info: TrashInfo): TrashPhase => (info.count === 0 ? "empty" : "list");

export function reduceTrash(s: TrashState, e: TrashEvent): TrashState {
  switch (e.type) {
    case "reload":
      return { ...initialTrash };
    case "loaded": {
      const info = infoOf(e.state);
      // Under og rett etter tømming beholdes ark/resultat; ny tilstand fra kjernen oppdaterer bare tallene.
      if (s.phase === "purging" || s.phase === "purged" || s.phase === "confirmPurge" || s.phase === "confirmPermanent") return { ...s, info };
      return { ...s, phase: listOf(info), info, error: null };
    }
    case "failed":
      return { ...s, phase: "error", error: e.error };
    case "openPurge":
      if (s.phase !== "list" || !s.info) return s;
      return { ...s, phase: "confirmPurge", mode: s.noBin ? "permanent" : "recycle", ack: false, error: null };
    case "setMode":
      return s.phase === "confirmPurge" && !(s.noBin && e.mode === "recycle") ? { ...s, mode: e.mode } : s;
    case "continue":
      if (s.phase !== "confirmPurge") return s;
      return s.mode === "permanent" ? { ...s, phase: "confirmPermanent", ack: false } : { ...s, phase: "purging" };
    case "setAck":
      return s.phase === "confirmPermanent" ? { ...s, ack: e.value } : s;
    case "confirmPermanent":
      return s.phase === "confirmPermanent" && s.ack ? { ...s, phase: "purging" } : s;
    case "cancel":
      return s.phase === "confirmPurge" || s.phase === "confirmPermanent" ? { ...s, phase: "list", ack: false } : s;
    case "purged": {
      const r = e.result;
      // Papirkurv-modus på volum uten Papirkurv: ingenting er flyttet. Tilbake til arket med bare permanent.
      if (e.mode === "recycle" && r.purged === 0 && r.unsupported > 0) {
        return { ...s, phase: "confirmPurge", noBin: true, mode: "permanent", ack: false };
      }
      return {
        ...s, phase: "purged", ack: false,
        outcome: { mode: e.mode, count: r.purged, marked: r.keptTrashed, freedBytes: r.freedBytes },
      };
    }
    case "purgeFailed":
      // Ingenting er slettet: tilbake til listen med feilmelding.
      return { ...s, phase: "list", ack: false, error: e.error };
    case "dismiss":
      return s.phase === "purged" ? { ...s, phase: s.info && s.info.count > 0 ? "list" : "empty", outcome: null } : s;
  }
}

/** Utløpsmerking: antall elementer som er utløpt (aldri stille slettet). */
export const hasExpired = (i: TrashInfo | null): boolean => !!i && i.expired > 0;
