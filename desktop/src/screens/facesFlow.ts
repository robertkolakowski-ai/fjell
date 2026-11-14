// Ansiktsindeks i Innstillinger: av/på med samtykke og «Slett ansiktsindeks» (kan ikke angres, bare etter bekreftelse).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";

export interface FacesState {
  phase: "loading" | "ready" | "error";
  enabled: boolean;
  /** Samtykkedialogen er åpen. */
  consent: boolean;
  /** Slettedialogen er åpen. */
  confirmWipe: boolean;
  busy: boolean;
  /** Antall ansikter slettet i siste wipe, til resultatmeldingen. */
  wiped: number | null;
  error: string | null;
}

export const initialFaces: FacesState = {
  phase: "loading", enabled: false, consent: false, confirmWipe: false, busy: false, wiped: null, error: null,
};

export class FacesSession {
  state: FacesState = initialFaces;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: FacesState) => void = () => undefined) {}

  dispose(): void { this.gone = true; }
  private set(patch: Partial<FacesState>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }
  private code(e: unknown): string { return (e as BridgeError)?.code ?? "core_error"; }

  async load(): Promise<void> {
    try {
      this.set({ phase: "ready", enabled: await this.b.getFaceIndex() });
    } catch {
      this.set({ phase: "error" });
    }
  }

  /** «På» krever samtykke: dialogen åpnes, og indeksen slås først på ved bekreftelse. */
  requestOn(): void { this.set({ consent: true, wiped: null, error: null }); }
  cancelConsent(): void { this.set({ consent: false }); }

  async confirmOn(): Promise<boolean> {
    if (this.state.busy) return false;
    this.set({ busy: true, error: null });
    try {
      await this.b.setFaceIndex(true);
      this.set({ busy: false, consent: false, enabled: true });
      return true;
    } catch (e) {
      this.set({ busy: false, consent: false, error: this.code(e) });
      return false;
    }
  }

  /** «Av» er trygt og krever ingen bekreftelse (slår bare av analysen; ingenting slettes). */
  async turnOff(): Promise<void> {
    if (this.state.busy || !this.state.enabled) return;
    this.set({ busy: true, error: null, wiped: null });
    try {
      await this.b.setFaceIndex(false);
      this.set({ busy: false, enabled: false });
    } catch (e) {
      this.set({ busy: false, error: this.code(e) });
    }
  }

  requestWipe(): void { this.set({ confirmWipe: true, wiped: null, error: null }); }
  cancelWipe(): void { if (!this.state.busy) this.set({ confirmWipe: false }); }

  /** Kaller `wipeFaceIndex` bare når bekreftelsesdialogen er åpen. */
  async confirmWipe(): Promise<boolean> {
    if (!this.state.confirmWipe || this.state.busy) return false;
    this.set({ busy: true, error: null });
    try {
      const wiped = await this.b.wipeFaceIndex();
      this.set({ busy: false, confirmWipe: false, wiped });
      return true;
    } catch (e) {
      this.set({ busy: false, confirmWipe: false, error: this.code(e) });
      return false;
    }
  }
}
