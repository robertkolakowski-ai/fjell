// Innstillinger: ViewState + ren reduce (PLAN-UX-I18N 3.x/4). Ingen I/O; effekter i settingsFlow.ts.
// Bare valg som ikke er skyvebrytere: språk (radio), papirkurvfrist (liste). «Tøm»-standard og karantenemappe vises, endres ikke her.
import type { AppSettings, BridgeError } from "../bridge/types";

export type SettingsPhase = "loading" | "ready" | "error";

export interface SettingsState {
  phase: SettingsPhase;
  settings: AppSettings | null;
  /** Lagring pågår. */
  saving: boolean;
  /** Siste lagring gikk bra (melding «Lagret»). */
  saved: boolean;
  /** Siste lagring feilet: innstillingen er uendret. */
  saveFailed: boolean;
  error: BridgeError | null;
}

export type SettingsEvent =
  | { type: "loaded"; settings: AppSettings }
  | { type: "failed"; error: BridgeError }
  | { type: "reload" }
  | { type: "saving" }
  | { type: "saved"; settings: AppSettings }
  | { type: "saveFailed" };

export const initialSettings: SettingsState = { phase: "loading", settings: null, saving: false, saved: false, saveFailed: false, error: null };

export function reduceSettings(s: SettingsState, e: SettingsEvent): SettingsState {
  switch (e.type) {
    case "reload":
      return { ...initialSettings };
    case "loaded":
      return { ...s, phase: "ready", settings: e.settings, error: null };
    case "failed":
      return { ...s, phase: "error", error: e.error };
    case "saving":
      return s.phase === "ready" && !s.saving ? { ...s, saving: true, saved: false, saveFailed: false } : s;
    case "saved":
      return s.saving ? { ...s, saving: false, saved: true, saveFailed: false, settings: e.settings } : s;
    case "saveFailed":
      return s.saving ? { ...s, saving: false, saved: false, saveFailed: true } : s;
  }
}

/** Valg i papirkurvfristen. Gjeldende verdi tas alltid med (også om den ikke står her), sortert og uten duplikater. */
export const TRASH_DAY_OPTIONS: readonly number[] = [7, 14, 30, 60, 90];
export const trashDayOptions = (current: number): number[] =>
  [...new Set([...TRASH_DAY_OPTIONS, current])].sort((a, b) => a - b);

/** Språkvalg i Innstillinger. «pseudo» er bare for tester og er ikke et brukervalg. */
export const LANGUAGE_CHOICES = ["system", "nb", "en"] as const;
export type LanguageChoice = (typeof LANGUAGE_CHOICES)[number];
