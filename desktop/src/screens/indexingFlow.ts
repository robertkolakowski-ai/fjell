// Indeksering: status (polling), pause, fortsett, avbryt og innstillinger. Skall i kontraktfasen
// (docs/v1/ROBUSTHET-KONTRAKT.md): grenen «indeksering» eier filen. Ingenting her berører bilder.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { IndexingSettings, IndexingStatus } from "../bridge/types-robust";

export const IDLE_STATUS: IndexingStatus = { state: "idle", phase: "idle", done: 0, total: 0, ratePerSec: 0, etaSecs: null };

export interface IndexingView {
  status: IndexingStatus;
  /** Statusen er hentet minst én gang (ellers vises ingenting, ikke «ingen indeksering»). */
  loaded: boolean;
  busy: boolean;
  error: string | null;
}

export const initialIndexing: IndexingView = { status: IDLE_STATUS, loaded: false, busy: false, error: null };

/** Gjenstående minutter, rundet opp (minst 1), eller null når det ikke kan anslås. */
export function etaMinutes(s: IndexingStatus): number | null {
  return s.etaSecs === null ? null : Math.max(1, Math.ceil(s.etaSecs / 60));
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

  async refresh(): Promise<void> {
    try {
      this.set({ status: await this.b.getIndexingStatus(), loaded: true, error: null });
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
      this.set({ busy: false, status: await this.b.getIndexingStatus() });
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
