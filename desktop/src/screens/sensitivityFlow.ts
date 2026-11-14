// Sensitivitet per forslagstype: tilstand og flyt mot CoreBridge (brukes av SettingsSensitivity.svelte, testes mot MockBridge).
// Appen foreslår bare: sensitivitet flytter bare hvor mange og hvor sikre nye forslag er. Terskelverdiene er skrivebeskyttet info.
import type { CoreBridge } from "../bridge/core-bridge";
import type { KindCode } from "../bridge/types";
import type { SensitivityCode, SuggestionSettings, Thresholds } from "../bridge/types-v1";

/** Typene der sensitivitet faktisk flytter terskler (V1-PLAN del 7). De andre har ingenting å justere. */
export const SENSITIVE_KINDS: readonly KindCode[] = ["visual_dup", "blurry", "series"];
export const SENSITIVITY_CODES: readonly SensitivityCode[] = ["low", "normal", "high"];

export type SensitivityPhase = "loading" | "ready" | "error";

export interface SensitivityState {
  phase: SensitivityPhase;
  settings: SuggestionSettings | null;
  thresholds: Thresholds | null;
  saving: boolean;
  saved: boolean;
  saveFailed: boolean;
}

export const initialSensitivity: SensitivityState = { phase: "loading", settings: null, thresholds: null, saving: false, saved: false, saveFailed: false };

export class SensitivitySession {
  state: SensitivityState = initialSensitivity;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: SensitivityState) => void = () => undefined) {}

  private set(patch: Partial<SensitivityState>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    try {
      const [settings, thresholds] = await Promise.all([this.b.getSuggestionSettings(), this.b.getThresholds()]);
      this.set({ phase: "ready", settings, thresholds });
    } catch {
      this.set({ phase: "error" });
    }
  }

  /** Endrer sensitivitet for én type. Samme verdi eller pågående lagring: ingen kall. Feil: verdien er uendret. */
  async choose(kind: KindCode, code: SensitivityCode): Promise<void> {
    const cur = this.state.settings;
    if (!cur || this.state.saving || cur.sensitivity[kind] === code) return;
    this.set({ saving: true, saved: false, saveFailed: false });
    try {
      const settings = await this.b.setSuggestionSettings({ ...cur, sensitivity: { ...cur.sensitivity, [kind]: code } });
      const thresholds = await this.b.getThresholds();
      this.set({ settings, thresholds, saving: false, saved: true });
    } catch {
      this.set({ saving: false, saveFailed: true });
    }
  }
}

/** Tall uten avrunding til heltall (uskarphetsgrensen er 0,006). Språkets desimaltegn. */
export const plainNumber = (n: number, intl: string): string => new Intl.NumberFormat(intl, { maximumFractionDigits: 4 }).format(n);
