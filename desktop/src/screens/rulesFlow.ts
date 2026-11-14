// Regler: flyt mot CoreBridge (brukes av Rules.svelte og RuleEditor.svelte, testes mot MockBridge).
// Regler foreslår bare. Sletting er mykt (`deleteRule`) og kan angres (`restoreRule`). Lagring krever en oppdatert forhåndsvisning.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { Condition, Rule, RulePreview } from "../bridge/types-v1";
import { conditionsKey, conditionsOf, newRow, rowFromCondition, type ConditionRow } from "./rulesState";

export type RulesPhase = "loading" | "ready" | "error";

/** Siste meldinger. `deleted` bærer id og navn så «Angre» kan hente regelen tilbake. */
export type RulesNote =
  | { kind: "saved"; name: string }
  | { kind: "toggled"; name: string; enabled: boolean }
  | { kind: "deleted"; id: number; name: string }
  | { kind: "restored"; name: string }
  | null;

export interface RulesState {
  phase: RulesPhase;
  rules: Rule[];
  note: RulesNote;
  /** Feil fra siste handling (ingenting er endret). */
  error: BridgeError | null;
  /** Regler det pågår en handling på (hindrer dobbeltklikk). */
  busy: ReadonlySet<number>;
}

export const initialRules: RulesState = { phase: "loading", rules: [], note: null, error: null, busy: new Set() };

export class RulesSession {
  state: RulesState = initialRules;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: RulesState) => void = () => undefined) {}

  private set(patch: Partial<RulesState>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    try {
      this.set({ phase: "ready", rules: await this.b.listRules(), error: null });
    } catch (e) {
      this.set({ phase: "error", error: e as BridgeError });
    }
  }

  private async refresh(): Promise<void> {
    try { this.set({ rules: await this.b.listRules() }); } catch { /* listen beholdes */ }
  }

  private async guarded(id: number, work: () => Promise<RulesNote>): Promise<boolean> {
    if (this.state.busy.has(id)) return false;
    this.set({ busy: new Set([...this.state.busy, id]), error: null });
    let ok = false;
    try {
      const note = await work();
      await this.refresh();
      this.set({ note });
      ok = true;
    } catch (e) {
      this.set({ error: e as BridgeError });
    }
    const busy = new Set(this.state.busy);
    busy.delete(id);
    this.set({ busy });
    return ok;
  }

  /** Av/på. Feil: bryteren står som før (listen leses på nytt). */
  async toggle(id: number, enabled: boolean): Promise<boolean> {
    const r = this.state.rules.find((x) => x.id === id);
    if (!r) return false;
    const ok = await this.guarded(id, async () => {
      await this.b.toggleRule(id, enabled);
      return { kind: "toggled", name: r.name, enabled };
    });
    if (!ok) await this.refresh();
    return ok;
  }

  /** Mykt slett. Meldingen `deleted` gir «Angre». Bildene røres ikke. */
  async remove(id: number): Promise<boolean> {
    const r = this.state.rules.find((x) => x.id === id);
    if (!r) return false;
    return this.guarded(id, async () => {
      await this.b.deleteRule(id);
      return { kind: "deleted", id, name: r.name };
    });
  }

  /** Angre sletting. */
  async undoRemove(): Promise<boolean> {
    const n = this.state.note;
    if (!n || n.kind !== "deleted") return false;
    return this.guarded(n.id, async () => {
      await this.b.restoreRule(n.id);
      return { kind: "restored", name: n.name };
    });
  }

  /** Editoren meldte at regelen er lagret. */
  async saved(name: string): Promise<void> {
    await this.refresh();
    this.set({ note: { kind: "saved", name }, error: null });
  }

  clearNote(): void { this.set({ note: null }); }
}

// ---- Regelbygger ----

export interface EditorState {
  id: number | null;
  name: string;
  rows: ConditionRow[];
  /** Vilkårene forhåndsvisningen gjelder for. null = ikke forhåndsvist. */
  previewedKey: string | null;
  preview: RulePreview | null;
  previewing: boolean;
  saving: boolean;
  /** Feil fra siste forhåndsvisning eller lagring, som kode (teksten kommer fra i18n). */
  error: BridgeError | null;
}

export function editorFor(rule: Rule | null): EditorState {
  return {
    id: rule?.id ?? null,
    name: rule?.name ?? "",
    rows: rule ? rule.conditions.map(rowFromCondition) : [newRow()],
    previewedKey: null, preview: null, previewing: false, saving: false, error: null,
  };
}

/** Forhåndsvisningen gjelder bare nøyaktig disse vilkårene. Endres noe, er den utdatert. */
export const previewIsFresh = (e: EditorState): boolean => e.preview !== null && e.previewedKey === conditionsKey(e.rows);
export const canSave = (e: EditorState): boolean => previewIsFresh(e) && !e.previewing && !e.saving;

/** Kjører forhåndsvisning (teller bare; lagrer ingenting). Feil (for eksempel `invalid_condition`) havner i `error`. */
export async function runPreview(b: CoreBridge, e: EditorState): Promise<EditorState> {
  const conditions: Condition[] = conditionsOf(e.rows);
  const key = JSON.stringify(conditions);
  try {
    const preview = await b.previewRule(conditions);
    return { ...e, preview, previewedKey: key, previewing: false, error: null };
  } catch (err) {
    return { ...e, preview: null, previewedKey: null, previewing: false, error: err as BridgeError };
  }
}

/** Lagrer regelen, men bare etter en oppdatert forhåndsvisning. Gir id ved suksess, ellers tilstand med feil. */
export async function saveEditor(b: CoreBridge, e: EditorState): Promise<{ id: number; state: EditorState } | { id: null; state: EditorState }> {
  if (!canSave({ ...e, saving: false })) return { id: null, state: e };
  try {
    const id = await b.saveRule({ id: e.id, name: e.name, conditions: conditionsOf(e.rows) });
    return { id, state: { ...e, saving: false, error: null } };
  } catch (err) {
    return { id: null, state: { ...e, saving: false, error: err as BridgeError } };
  }
}
