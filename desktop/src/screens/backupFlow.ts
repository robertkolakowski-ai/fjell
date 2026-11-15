// Sikkerhetskopi av indeksen i Innstillinger og i trygg modus: liste, «Ta kopi nå», gjenoppretting med avkrysning og angre.
// Skall i kontraktfasen (docs/v1/ROBUSTHET-KONTRAKT.md): grenen «backup» eier filen. Ingenting her berører bilder.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { Backup, IntegrityCode, RestoreReport } from "../bridge/types-robust";

export interface BackupState {
  phase: "loading" | "ready" | "error";
  backups: Backup[];
  integrity: IntegrityCode;
  busy: boolean;
  /** Kopien bekreftelsesdialogen gjelder (åpen når satt). */
  confirmId: number | null;
  /** Avkrysningen i dialogen. «Gjenopprett» virker ikke uten den. */
  ack: boolean;
  /** Siste gjenoppretting (viser «Angre»). Nullstilles ved ny handling. */
  restored: RestoreReport | null;
  created: boolean;
  /** Feilkode (`BridgeError.code`) til `error.<kode>`. */
  error: string | null;
}

/** Kan siste gjenoppretting angres? Gjenoppretting uten åpen indeks (trygg modus) har ingen kopi av «før» (`safety` = 0):
 * den ødelagte indeksen er lagt til side i datamappen, ikke slettet, men kan ikke settes tilbake fra appen. */
export function canUndo(r: RestoreReport | null): boolean {
  return r !== null && r.safety > 0;
}

export const initialBackup: BackupState = {
  phase: "loading", backups: [], integrity: "ok", busy: false, confirmId: null, ack: false, restored: null, created: false, error: null,
};

export class BackupSession {
  state: BackupState = initialBackup;
  private gone = false;

  /** `onChange` får hver ny tilstand; `onIndexChanged` kalles etter at indeksen kan ha endret seg (gjenoppretting, angre, sjekk). */
  constructor(
    private readonly b: CoreBridge,
    private readonly onChange: (s: BackupState) => void = () => undefined,
    private readonly onIndexChanged: () => void = () => undefined,
  ) {}

  dispose(): void { this.gone = true; }
  private set(patch: Partial<BackupState>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }
  private code(e: unknown): string { return (e as BridgeError)?.code ?? "core_error"; }

  async load(): Promise<void> {
    try {
      const [backups, integrity] = await Promise.all([this.b.listBackups(), this.b.checkIntegrity()]);
      this.set({ phase: "ready", backups, integrity });
    } catch {
      this.set({ phase: "error" });
    }
  }

  async create(): Promise<void> {
    if (this.state.busy) return;
    this.set({ busy: true, error: null, created: false, restored: null });
    try {
      await this.b.createBackup();
      this.set({ busy: false, created: true, backups: await this.b.listBackups() });
    } catch (e) {
      this.set({ busy: false, error: this.code(e) });
    }
  }

  async check(): Promise<void> {
    this.set({ busy: true, error: null });
    try {
      this.set({ busy: false, integrity: await this.b.checkIntegrity() });
      this.onIndexChanged();
    } catch (e) {
      this.set({ busy: false, error: this.code(e) });
    }
  }

  requestRestore(id: number): void { this.set({ confirmId: id, ack: false, error: null, created: false }); }
  cancelRestore(): void { this.set({ confirmId: null, ack: false }); }
  setAck(ack: boolean): void { this.set({ ack }); }

  /** Eneste vei til `restoreBackup`: avkrysning må være på. Ingen avkrysning, ingen kall. */
  async confirmRestore(): Promise<boolean> {
    const { confirmId, ack, busy } = this.state;
    if (confirmId === null || !ack || busy) return false;
    this.set({ busy: true, error: null });
    try {
      const restored = await this.b.restoreBackup(confirmId, true);
      const [backups, integrity] = await Promise.all([this.b.listBackups(), this.b.checkIntegrity()]);
      this.set({ busy: false, confirmId: null, ack: false, restored, backups, integrity });
      this.onIndexChanged();
      return true;
    } catch (e) {
      this.set({ busy: false, confirmId: null, ack: false, error: this.code(e) });
      return false;
    }
  }

  /** Angrer siste gjenoppretting (setter `safety` tilbake). */
  async undoRestore(): Promise<void> {
    const r = this.state.restored;
    if (!r || !canUndo(r) || this.state.busy) return;
    this.set({ busy: true, error: null });
    try {
      await this.b.undoRestoreBackup(r.safety);
      const [backups, integrity] = await Promise.all([this.b.listBackups(), this.b.checkIntegrity()]);
      this.set({ busy: false, restored: null, backups, integrity });
      this.onIndexChanged();
    } catch (e) {
      this.set({ busy: false, error: this.code(e) });
    }
  }
}
