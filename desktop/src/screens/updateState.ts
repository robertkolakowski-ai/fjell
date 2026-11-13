// Oppdateringer (M9): ViewState + ren reduce. Ingen I/O; effekter i updateFlow.ts.
// Personvern: sjekk skjer bare på knappetrykk, eller ved oppstart når «Sjekk automatisk» er på. Aldri automatisk installasjon.
import type { AppState, BridgeError, UpdateCheck } from "../bridge/types";

export type UpdateStatus = "idle" | "checking" | "none" | "available" | "error" | "installing" | "restarting" | "installFailed";

export interface UpdateView {
  status: UpdateStatus;
  /** Installert versjon, eller null til den er lest. */
  version: string | null;
  /** Funnet oppdatering (status available, installing, installFailed). */
  found: (UpdateCheck & { version: string }) | null;
  error: BridgeError | null;
  done: number;
  total: number | null;
  /** «Sjekk automatisk ved oppstart». Standard av. */
  auto: boolean;
  autoSaving: boolean;
  autoFailed: boolean;
  /** Skanning pågår (kjører, ikke pauset). Installasjon er da blokkert. */
  scanning: boolean;
}

export type UpdateEvent =
  | { type: "version"; version: string }
  | { type: "auto"; on: boolean }
  | { type: "autoSaving" }
  | { type: "autoFailed" }
  | { type: "scan"; scanning: boolean }
  | { type: "checking" }
  | { type: "checked"; result: UpdateCheck }
  | { type: "checkFailed"; error: BridgeError }
  | { type: "installing" }
  | { type: "progress"; done: number; total: number | null }
  | { type: "restarting" }
  | { type: "installFailed"; error: BridgeError };

export const initialUpdate: UpdateView = {
  status: "idle", version: null, found: null, error: null, done: 0, total: null,
  auto: false, autoSaving: false, autoFailed: false, scanning: false,
};

const busy = (s: UpdateView): boolean => s.status === "checking" || s.status === "installing" || s.status === "restarting";

export function reduceUpdate(s: UpdateView, e: UpdateEvent): UpdateView {
  switch (e.type) {
    case "version":
      return { ...s, version: e.version };
    case "auto":
      return { ...s, auto: e.on, autoSaving: false, autoFailed: false };
    case "autoSaving":
      return s.autoSaving ? s : { ...s, autoSaving: true, autoFailed: false };
    case "autoFailed":
      return { ...s, autoSaving: false, autoFailed: true };
    case "scan":
      return { ...s, scanning: e.scanning };
    case "checking":
      return busy(s) ? s : { ...s, status: "checking", error: null };
    case "checked":
      if (s.status !== "checking") return s;
      return e.result.available && e.result.version
        ? { ...s, status: "available", found: { ...e.result, version: e.result.version }, error: null }
        : { ...s, status: "none", found: null, error: null };
    case "checkFailed":
      return s.status === "checking" ? { ...s, status: "error", found: null, error: e.error } : s;
    case "installing":
      // Bare fra «available» eller etter en mislykket installasjon, og aldri mens skanning pågår.
      return (s.status === "available" || s.status === "installFailed") && s.found && !s.scanning
        ? { ...s, status: "installing", done: 0, total: null, error: null }
        : s;
    case "progress":
      return s.status === "installing" ? { ...s, done: e.done, total: e.total } : s;
    case "restarting":
      return s.status === "installing" ? { ...s, status: "restarting" } : s;
    case "installFailed":
      return s.status === "installing" ? { ...s, status: "installFailed", error: e.error } : s;
  }
}

/** Skanning kjører (ikke pauset eller ferdig). Pauset skann huskes av kjernen og er trygt å starte på nytt over. */
export const isScanning = (st: Pick<AppState, "scanPhase" | "scanPaused">): boolean => st.scanPhase === "analyzing" && !st.scanPaused;

/** «Installer og start på nytt» er blokkert (men synlig, med forklaring). */
export const installBlocked = (s: UpdateView): boolean => s.scanning && (s.status === "available" || s.status === "installFailed");

/** Fremdrift i hele prosent, eller null når total er ukjent. */
export function progressPercent(done: number, total: number | null): number | null {
  if (total === null || total <= 0) return null;
  return Math.max(0, Math.min(100, Math.floor((done / total) * 100)));
}
