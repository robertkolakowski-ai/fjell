// Beskyttelser: tilstand og flyt mot CoreBridge (brukes av SettingsProtections.svelte, testes mot MockBridge).
// Å fjerne en beskyttelse rører aldri bildene: den krever bekreftelse og kan angres (`protect(scope, true)`).
// Frontend får ingen ny filtilgang: mappestien kommer fra tekstfelt eller eksisterende mappevalg (`browseFolder`).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { Protection, Scope } from "../bridge/types-v1";
import type { TFunction } from "../lib/i18n";

export type ProtectionsPhase = "loading" | "ready" | "error";

export type ProtectionsNote =
  | { kind: "added"; scope: Scope }
  | { kind: "removed"; scope: Scope }
  | { kind: "restored"; scope: Scope }
  | null;

export interface ProtectionsState {
  phase: ProtectionsPhase;
  items: Protection[];
  /** Beskyttelsen brukeren er i ferd med å fjerne (venter på bekreftelse). */
  pending: Scope | null;
  note: ProtectionsNote;
  busy: boolean;
  failed: boolean;
}

export const initialProtections: ProtectionsState = { phase: "loading", items: [], pending: null, note: null, busy: false, failed: false };

export const sameScope = (a: Scope, b: Scope): boolean => a.scope === b.scope && a.target === b.target;

/** Kort beskrivelse av hva som er beskyttet: mappestien, ellers «nr. N». Navn på personer lagres ikke her. */
export function describeTarget(s: Scope, t: TFunction): string {
  return s.scope === "folder" ? s.target : t("protect.target.id", { id: s.target });
}

export class ProtectionsSession {
  state: ProtectionsState = initialProtections;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: ProtectionsState) => void = () => undefined) {}

  private set(patch: Partial<ProtectionsState>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    try {
      this.set({ phase: "ready", items: await this.b.listProtections() });
    } catch {
      this.set({ phase: "error" });
    }
  }

  private async reload(): Promise<void> {
    try { this.set({ items: await this.b.listProtections() }); } catch { /* listen beholdes */ }
  }

  /** Åpner bekreftelsen. Ingenting endres ennå. */
  askRemove(scope: Scope): void { this.set({ pending: scope, failed: false }); }
  cancelRemove(): void { this.set({ pending: null }); }

  /** Bekreftet fjerning. Feil: beskyttelsen står som før. */
  async confirmRemove(): Promise<boolean> {
    const scope = this.state.pending;
    if (!scope || this.state.busy) return false;
    this.set({ busy: true, failed: false });
    try {
      await this.b.protect(scope, false);
      await this.reload();
      this.set({ busy: false, pending: null, note: { kind: "removed", scope } });
      return true;
    } catch {
      this.set({ busy: false, pending: null, failed: true });
      return false;
    }
  }

  /** Angre fjerning: setter beskyttelsen på igjen. */
  async undoRemove(): Promise<boolean> {
    const n = this.state.note;
    if (!n || n.kind !== "removed" || this.state.busy) return false;
    this.set({ busy: true, failed: false });
    try {
      await this.b.protect(n.scope, true);
      await this.reload();
      this.set({ busy: false, note: { kind: "restored", scope: n.scope } });
      return true;
    } catch {
      this.set({ busy: false, failed: true });
      return false;
    }
  }

  /** Beskytter en mappe. Tom sti avvises uten kall. Gir `ok: false` med feilkoden når kjernen avviser. */
  async addFolder(raw: string): Promise<{ ok: true } | { ok: false; error: BridgeError }> {
    const target = raw.trim();
    if (target === "") return { ok: false, error: { code: "empty_folder", detail: "" } };
    const scope: Scope = { scope: "folder", target };
    try {
      await this.b.protect(scope, true);
    } catch (e) {
      return { ok: false, error: e as BridgeError };
    }
    await this.reload();
    this.set({ note: { kind: "added", scope: { scope: "folder", target } }, failed: false });
    return { ok: true };
  }

  clearNote(): void { this.set({ note: null }); }
}
