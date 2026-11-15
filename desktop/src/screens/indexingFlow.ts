// Indeksering: status (polling), pause, fortsett, avbryt og innstillinger (docs/v1/INDEKSERING.md). Ingenting her berører
// bilder; alt går via CoreBridge. Statusen hentes bare mens vinduet er synlig, og tilstandsendringer gir én aria-live-melding.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { IndexingSettings, IndexingStatus } from "../bridge/types-robust";
import type { MessageKey } from "../lib/i18n";

export const IDLE_STATUS: IndexingStatus = { state: "idle", phase: "idle", done: 0, total: 0, ratePerSec: 0, etaSecs: null };

export interface IndexingView {
  status: IndexingStatus;
  /** Statusen er hentet minst én gang (ellers vises ingenting, ikke «ingen indeksering»). */
  loaded: boolean;
  busy: boolean;
  error: string | null;
  /** Melding til aria-live ved tilstandsendring (null = ingenting nytt). Settes bare når tilstanden endres. */
  announce: MessageKey | null;
}

export const initialIndexing: IndexingView = { status: IDLE_STATUS, loaded: false, busy: false, error: null, announce: null };

/** Gjenstående minutter, rundet opp (minst 1), eller null når det ikke kan anslås. */
export function etaMinutes(s: IndexingStatus): number | null {
  return s.etaSecs === null ? null : Math.max(1, Math.ceil(s.etaSecs / 60));
}

/** Antall bilder som gjenstår (aldri negativt). */
export function remaining(s: IndexingStatus): number {
  return Math.max(0, s.total - s.done);
}

/** Tekstnøkkel for tilstanden. Ukjent kode (f.eks. `disk_offline` før den delte typen får den) vises som pause. */
export function stateKey(state: string): MessageKey {
  switch (state) {
    case "running": return "indexing.state.running";
    case "paused": return "indexing.state.paused";
    case "paused_on_battery": return "indexing.state.paused_on_battery";
    case "disk_offline": return "indexing.state.disk_offline";
    case "idle": return "indexing.state.idle";
    default: return "indexing.state.paused";
  }
}

/** Rolig forklaring under tilstanden (ingen skremmende tekst), eller null. */
export function hintKey(state: string): MessageKey | null {
  if (state === "paused_on_battery") return "indexing.hint.battery";
  if (state === "disk_offline") return "indexing.hint.disk_offline";
  return null;
}

/** Står indekseringen stille uten at brukeren har bedt om det? Da er «Fortsett» fortsatt en gyldig knapp. */
export function isHalted(state: string): boolean {
  return state !== "idle" && state !== "running";
}

/**
 * Hva skjermleseren skal få høre når tilstanden endres (aria-live). `prev` er forrige kjente tilstand (null =
 * første måling). Første måling kunngjør bare stille tilstander (ikke «Indekserer …» for noe som alt pågikk).
 */
export function announceKey(prev: string | null, next: string): MessageKey | null {
  if (prev === next) return null;
  if (next === "idle") return prev !== null && prev !== "idle" ? "indexing.live.done" : null;
  if (next === "running" && prev === null) return null;
  return stateKey(next);
}

export class IndexingSession {
  state: IndexingView = initialIndexing;
  private gone = false;
  private timer: ReturnType<typeof setInterval> | undefined;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: IndexingView) => void = () => undefined) {}

  private set(patch: Partial<IndexingView>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }
  private code(e: unknown): string { return (e as BridgeError)?.code ?? "core_error"; }

  /** Tar imot en ny status. Kunngjør bare endringer i tilstanden (ikke hver polling). */
  private take(status: IndexingStatus): Partial<IndexingView> {
    const prev = this.state.loaded ? this.state.status.state : null;
    const key = announceKey(prev, status.state);
    return key === null ? { status, loaded: true } : { status, loaded: true, announce: key };
  }

  async refresh(): Promise<void> {
    try {
      this.set({ ...this.take(await this.b.getIndexingStatus()), error: null });
    } catch (e) {
      this.set({ error: this.code(e) });
    }
  }

  /** Henter status nå og deretter hvert `ms`, bare mens vinduet er synlig. */
  start(ms = 2000): void {
    void this.refresh();
    this.timer = setInterval(() => { if (typeof document === "undefined" || !document.hidden) void this.refresh(); }, ms);
  }

  dispose(): void {
    this.gone = true;
    clearInterval(this.timer);
  }

  private async act(fn: () => Promise<void>): Promise<void> {
    if (this.state.busy) return;
    this.set({ busy: true, error: null });
    try {
      await fn();
      this.set({ busy: false, ...this.take(await this.b.getIndexingStatus()) });
    } catch (e) {
      this.set({ busy: false, error: this.code(e) });
    }
  }
  pause(): Promise<void> { return this.act(() => this.b.pauseIndexing()); }
  resume(): Promise<void> { return this.act(() => this.b.resumeIndexing()); }
  cancel(): Promise<void> { return this.act(() => this.b.cancelIndexing()); }
}

export interface IndexingSettingsView {
  phase: "loading" | "ready" | "error";
  settings: IndexingSettings | null;
  saving: boolean;
  error: string | null;
}

export const initialIndexingSettings: IndexingSettingsView = { phase: "loading", settings: null, saving: false, error: null };

export class IndexingSettingsSession {
  state: IndexingSettingsView = initialIndexingSettings;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: IndexingSettingsView) => void = () => undefined) {}

  dispose(): void { this.gone = true; }
  private set(patch: Partial<IndexingSettingsView>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }

  async load(): Promise<void> {
    try {
      this.set({ phase: "ready", settings: await this.b.getIndexingSettings() });
    } catch {
      this.set({ phase: "error" });
    }
  }

  /** Lagrer ett felt av gangen; viser det kjernen faktisk lagret. */
  async update(patch: Partial<IndexingSettings>): Promise<void> {
    const cur = this.state.settings;
    if (!cur || this.state.saving) return;
    this.set({ saving: true, error: null });
    try {
      this.set({ saving: false, settings: await this.b.setIndexingSettings({ ...cur, ...patch }) });
    } catch (e) {
      this.set({ saving: false, error: (e as BridgeError)?.code ?? "core_error" });
    }
  }
}
