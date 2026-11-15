// Diagnose: forhåndsvisning av rapporten og «Lagre rapport» (docs/v1/DIAGNOSE.md). Rapporten har bare tall og koder; appen
// sender den aldri noe sted. Eksporten lagres av skallet i appens diagnosemappe; hvis svaret har med stien (`savedPath`),
// vises den så brukeren selv kan sende filen. Forhåndsvisningen er nøyaktig teksten som lagres.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { DiagReport } from "../bridge/types-robust";

export interface DiagView {
  phase: "loading" | "ready" | "error";
  report: DiagReport | null;
  busy: boolean;
  /** Rapporten er laget (eksportert) i denne økten. */
  exported: boolean;
  /** Hvor filen ble lagret, hvis skallet sa det. Bare vist til brukeren. */
  savedPath: string | null;
  error: string | null;
}

export const initialDiag: DiagView = { phase: "loading", report: null, busy: false, exported: false, savedPath: null, error: null };

export class DiagSession {
  state: DiagView = initialDiag;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: DiagView) => void = () => undefined) {}

  dispose(): void { this.gone = true; }
  private set(patch: Partial<DiagView>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }

  async load(): Promise<void> {
    try {
      this.set({ phase: "ready", report: await this.b.previewDiagnostics() });
    } catch {
      this.set({ phase: "error" });
    }
  }

  async exportReport(): Promise<void> {
    if (this.state.busy) return;
    this.set({ busy: true, error: null, exported: false, savedPath: null });
    try {
      const r: DiagReport & { savedPath?: unknown } = await this.b.exportDiagnostics();
      const savedPath = typeof r.savedPath === "string" && r.savedPath !== "" ? r.savedPath : null;
      this.set({ busy: false, exported: true, savedPath, report: r });
    } catch (e) {
      this.set({ busy: false, error: (e as BridgeError)?.code ?? "core_error" });
    }
  }
}
