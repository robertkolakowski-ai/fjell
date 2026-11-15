// Diagnose: forhåndsvisning av rapporten og «Lag rapport». Skall i kontraktfasen (docs/v1/ROBUSTHET-KONTRAKT.md): grenen
// «diagnose» eier filen. Rapporten har bare tall og koder; appen sender den aldri noe sted. Hvor eksporten havner er et åpent spørsmål.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { DiagReport } from "../bridge/types-robust";

export interface DiagView {
  phase: "loading" | "ready" | "error";
  report: DiagReport | null;
  busy: boolean;
  /** Rapporten er laget (eksportert) i denne økten. */
  exported: boolean;
  error: string | null;
}

export const initialDiag: DiagView = { phase: "loading", report: null, busy: false, exported: false, error: null };

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
    this.set({ busy: true, error: null, exported: false });
    try {
      this.set({ busy: false, exported: true, report: await this.b.exportDiagnostics() });
    } catch (e) {
      this.set({ busy: false, error: (e as BridgeError)?.code ?? "core_error" });
    }
  }
}
