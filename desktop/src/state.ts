// Ren tilstandsmaskin: skann → gjennomgang → bekreftelse → papirkurv/angre.
// Ingen I/O: effekter (bridge-kall) gjøres av kalleren, som sender resultat tilbake som events.
import type { BridgeError, Card, ScanPhase } from "./bridge/types";

export type Step = "idle" | "scanning" | "review" | "confirm" | "working" | "trashed";

export interface UiState {
  step: Step;
  scan: { phase: ScanPhase; done: number; total: number };
  selected: ReadonlySet<number>;
  /** Siste papirkurv-batch som kan angres. */
  undoBatch: number | null;
  /** Kort i siste batch (til «Angre»-tekst). */
  lastCount: number;
  error: BridgeError | null;
}

export type Event =
  | { type: "scanStart" }
  | { type: "scanProgress"; phase: ScanPhase; done: number; total: number }
  | { type: "scanFailed"; error: BridgeError }
  | { type: "toggle"; id: number }
  | { type: "selectAll"; cards: readonly Card[] }
  | { type: "clearSelection" }
  | { type: "requestConfirm" }
  | { type: "cancelConfirm" }
  | { type: "confirm" }
  | { type: "trashed"; batch: number }
  | { type: "trashFailed"; error: BridgeError }
  | { type: "undo" }
  | { type: "undone" }
  | { type: "undoFailed"; error: BridgeError }
  | { type: "dismissUndo" }
  | { type: "backToReview" };

export const initialState: UiState = {
  step: "idle",
  scan: { phase: "idle", done: 0, total: 0 },
  selected: new Set(),
  undoBatch: null,
  lastCount: 0,
  error: null,
};

export function reduce(s: UiState, e: Event): UiState {
  switch (e.type) {
    case "scanStart":
      if (s.step !== "idle" && s.step !== "review" && s.step !== "trashed") return s;
      return { ...initialState, step: "scanning", scan: { phase: "listing", done: 0, total: 0 } };
    case "scanProgress":
      if (s.step !== "scanning") return s;
      return {
        ...s,
        scan: { phase: e.phase, done: e.done, total: e.total },
        step: e.phase === "done" ? "review" : "scanning",
      };
    case "scanFailed":
      return s.step === "scanning" ? { ...s, step: "idle", error: e.error } : s;
    case "toggle": {
      if (s.step !== "review") return s;
      const next = new Set(s.selected);
      if (!next.delete(e.id)) next.add(e.id);
      return { ...s, selected: next };
    }
    case "selectAll":
      return s.step === "review" ? { ...s, selected: new Set(e.cards.map((c) => c.id)) } : s;
    case "clearSelection":
      return s.step === "review" ? { ...s, selected: new Set() } : s;
    case "requestConfirm":
      // Ingenting valgt: ingen bekreftelse (aldri tom handling).
      return s.step === "review" && s.selected.size > 0 ? { ...s, step: "confirm", error: null } : s;
    case "cancelConfirm":
      return s.step === "confirm" ? { ...s, step: "review" } : s;
    case "confirm":
      return s.step === "confirm" ? { ...s, step: "working" } : s;
    case "trashed":
      return s.step === "working"
        ? { ...s, step: "trashed", undoBatch: e.batch, lastCount: s.selected.size, selected: new Set() }
        : s;
    case "trashFailed":
      return s.step === "working" ? { ...s, step: "review", error: e.error } : s;
    case "undo":
      return s.step === "trashed" && s.undoBatch !== null ? { ...s, step: "working" } : s;
    case "undone":
      return s.step === "working" && s.undoBatch !== null
        ? { ...s, step: "review", undoBatch: null, lastCount: 0 }
        : s;
    case "undoFailed":
      return s.step === "working" ? { ...s, step: "trashed", error: e.error } : s;
    case "dismissUndo":
      return s.step === "trashed" ? { ...s, undoBatch: null } : s;
    case "backToReview":
      return s.step === "trashed" ? { ...s, step: "review", undoBatch: null } : s;
  }
}
