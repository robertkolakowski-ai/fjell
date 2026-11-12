// Onboarding: ViewState + ren reduce (PLAN-UX-I18N del 3.1 og 4). Ingen I/O; effekter ligger i onboardingFlow.ts.

/** MassAck kreves ved >= 50 bilder eller >= 1 GB (TRYGGHET 4.3, PLAN 3.1 trinn 4). 1 GB = 1024^3 som i formatBytes. */
export const MASS_COUNT = 50;
export const MASS_BYTES = 1024 ** 3;
export const needsMassAck = (count: number, bytes: number): boolean => count >= MASS_COUNT || bytes >= MASS_BYTES;

export type OnbState =
  | { kind: "welcome" }
  | { kind: "chooseFolders"; failed: boolean }
  | { kind: "noAccess" }
  | { kind: "scanning"; found: number }
  | { kind: "partial"; found: number }
  | { kind: "result"; count: number; bytes: number; ids: number[]; confirming: boolean; acked: boolean; busy: boolean; failed: boolean }
  | { kind: "nothingFound" }
  | { kind: "done"; count: number; bytes: number; ids: number[]; busy: boolean; failed: boolean };

export type OnbEvent =
  | { type: "begin" }
  | { type: "startScan" }
  | { type: "chooseAgain" }
  | { type: "scanDenied" }
  | { type: "scanFailed" }
  | { type: "progress"; found: number }
  | { type: "slow" }
  | { type: "exactReady"; count: number; bytes: number; ids: number[] }
  | { type: "requestTrash" }
  | { type: "setAck"; acked: boolean }
  | { type: "cancelConfirm" }
  | { type: "confirmTrash" }
  | { type: "trashed" }
  | { type: "trashFailed" }
  | { type: "undo" }
  | { type: "undone" }
  | { type: "undoFailed" };

export const initialOnb: OnbState = { kind: "welcome" };

export function reduceOnb(s: OnbState, e: OnbEvent): OnbState {
  switch (e.type) {
    case "begin":
      return s.kind === "welcome" ? { kind: "chooseFolders", failed: false } : s;
    case "startScan":
      return s.kind === "chooseFolders" ? { kind: "scanning", found: 0 } : s;
    case "chooseAgain":
      return s.kind === "noAccess" ? { kind: "chooseFolders", failed: false } : s;
    case "scanDenied":
      return s.kind === "scanning" ? { kind: "noAccess" } : s;
    case "scanFailed":
      return s.kind === "scanning" || s.kind === "partial" ? { kind: "chooseFolders", failed: true } : s;
    case "progress":
      return s.kind === "scanning" || s.kind === "partial" ? { ...s, found: e.found } : s;
    case "slow":
      return s.kind === "scanning" ? { kind: "partial", found: s.found } : s;
    case "exactReady":
      if (s.kind !== "scanning" && s.kind !== "partial") return s;
      return e.count === 0
        ? { kind: "nothingFound" }
        : { kind: "result", count: e.count, bytes: e.bytes, ids: e.ids, confirming: false, acked: false, busy: false, failed: false };
    case "requestTrash":
      if (s.kind !== "result" || s.busy || s.confirming) return s;
      return needsMassAck(s.count, s.bytes)
        ? { ...s, confirming: true, acked: false, failed: false }
        : { ...s, busy: true, failed: false };
    case "setAck":
      return s.kind === "result" && s.confirming ? { ...s, acked: e.acked } : s;
    case "cancelConfirm":
      return s.kind === "result" && s.confirming ? { ...s, confirming: false, acked: false } : s;
    case "confirmTrash":
      // Aldri uten avkrysning.
      return s.kind === "result" && s.confirming && s.acked ? { ...s, confirming: false, busy: true } : s;
    case "trashed":
      return s.kind === "result" && s.busy
        ? { kind: "done", count: s.count, bytes: s.bytes, ids: s.ids, busy: false, failed: false }
        : s;
    case "trashFailed":
      return s.kind === "result" && s.busy ? { ...s, busy: false, failed: true } : s;
    case "undo":
      return s.kind === "done" && !s.busy ? { ...s, busy: true, failed: false } : s;
    case "undone":
      return s.kind === "done" && s.busy
        ? { kind: "result", count: s.count, bytes: s.bytes, ids: s.ids, confirming: false, acked: false, busy: false, failed: false }
        : s;
    case "undoFailed":
      return s.kind === "done" && s.busy ? { ...s, busy: false, failed: true } : s;
  }
}
