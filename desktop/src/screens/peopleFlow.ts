// Personer: flyt mot CoreBridge (brukes av People.svelte, testes mot MockBridge).
// Ingenting her sletter bilder. Sammenslåing, beskyttelse og navn kan angres (`undo`).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import type { Person } from "../bridge/types-v1";
import { defaultKeep, displayName, initialPeople, mergeRoles, validateName, type PeopleState, type UndoAction } from "./peopleState";

/** Tekster som flyten trenger (kommer fra i18n i skjermen; tester gir faste strenger). */
export interface PeopleLabels {
  unnamed: string;
}

export class PeopleSession {
  state: PeopleState = initialPeople;
  private gone = false;

  constructor(
    private readonly b: CoreBridge,
    private readonly labels: () => PeopleLabels,
    private readonly onChange: (s: PeopleState) => void = () => undefined,
  ) {}

  dispose(): void { this.gone = true; }

  private set(patch: Partial<PeopleState>): void {
    if (this.gone) return;
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }

  label(p: Person | undefined): string {
    return p ? displayName(p, this.labels().unnamed) : "";
  }
  private find(id: number): Person | undefined { return this.state.people.find((p) => p.id === id); }

  async load(): Promise<void> {
    try {
      const faceIndex = await this.b.getFaceIndex();
      const people = faceIndex ? await this.b.listPeople() : [];
      this.set({ phase: "ready", faceIndex, people, error: null });
    } catch {
      this.set({ phase: "error" });
    }
  }

  private fail(e: unknown): void {
    this.set({ busy: false, error: (e as BridgeError)?.code ?? "core_error" });
  }

  clearFocus(): void { if (this.state.focusId !== null) this.set({ focusId: null }); }

  // ---- Samtykke og av/på ----
  openConsent(): void { this.set({ consent: true }); }
  closeConsent(): void { this.set({ consent: false }); }

  /** Slår på ansiktsindeksen. Kalles bare fra samtykkedialogen. */
  async enable(): Promise<boolean> {
    if (this.state.busy) return false;
    this.set({ busy: true, error: null });
    try {
      await this.b.setFaceIndex(true);
      const people = await this.b.listPeople();
      this.set({ busy: false, consent: false, faceIndex: true, people, phase: "ready" });
      return true;
    } catch (e) {
      this.fail(e);
      this.set({ consent: false });
      return false;
    }
  }

  // ---- Navn ----
  startRename(id: number): void { this.set({ editing: id, nameError: null }); }
  cancelRename(): void { this.set({ editing: null, nameError: null, focusId: this.state.editing }); }

  async saveName(id: number, raw: string): Promise<void> {
    const person = this.find(id);
    if (!person || this.state.busy) return;
    const next = raw.trim() === "" ? null : raw.trim();
    if (next === person.name) { this.cancelRename(); return; }
    const bad = validateName(raw);
    if (bad) { this.set({ nameError: bad }); return; }
    this.set({ busy: true, nameError: null });
    try {
      await this.b.namePerson(id, next);
      const people = this.state.people.map((p) => (p.id === id ? { ...p, name: next } : p));
      this.set({
        busy: false, editing: null, people, focusId: id, error: null,
        undo: { kind: "name", id, prev: person.name, label: this.label({ ...person, name: next }) },
      });
    } catch (e) {
      const code = (e as BridgeError)?.code ?? "core_error";
      this.set({ busy: false, nameError: code });
    }
  }

  // ---- Sammenslåing ----
  openMerge(from: number): void {
    const others = this.state.people.filter((p) => p.id !== from);
    if (!this.find(from)) return;
    this.set({ merge: { from, other: others.length === 1 ? others[0].id : null, keep: "from" } });
    if (others.length === 1) this.pickMergeOther(others[0].id);
  }
  cancelMerge(): void { this.set({ merge: null }); }

  pickMergeOther(other: number): void {
    const m = this.state.merge, a = m && this.find(m.from), c = this.find(other);
    if (!m || !a || !c) return;
    this.set({ merge: { ...m, other, keep: defaultKeep(a, c) } });
  }
  setMergeKeep(keep: "from" | "other"): void {
    const m = this.state.merge;
    if (m) this.set({ merge: { ...m, keep } });
  }

  async confirmMerge(): Promise<boolean> {
    const m = this.state.merge;
    const roles = m && mergeRoles(m);
    if (!m || !roles || this.state.busy) return false;
    const absorbed = this.find(roles.absorbed), target = this.find(roles.target);
    this.set({ busy: true, error: null });
    try {
      const kept = await this.b.mergePeople(roles.target, roles.absorbed);
      const people = await this.b.listPeople();
      this.set({
        busy: false, merge: null, people, focusId: kept,
        undo: { kind: "merge", absorbed: roles.absorbed, target: kept, absorbedLabel: this.label(absorbed), targetLabel: this.label(target) },
      });
      return true;
    } catch (e) {
      this.fail(e);
      this.set({ merge: null });
      return false;
    }
  }

  // ---- Beskyttelse ----
  async toggleProtect(id: number, next: boolean): Promise<void> {
    const person = this.find(id);
    if (!person || this.state.busy) return;
    this.set({ busy: true, error: null });
    try {
      await this.b.protectPerson(id, next);
      const people = this.state.people.map((p) => (p.id === id ? { ...p, protected: next } : p));
      this.set({ busy: false, people, undo: { kind: "protect", id, prev: person.protected, label: this.label(person) } });
    } catch (e) {
      this.fail(e);
    }
  }

  // ---- Angre ----
  dismissUndo(): void { this.set({ undo: null }); }

  /** Angrer siste handling. Gir true hvis noe ble angret. */
  async undo(): Promise<boolean> {
    const u: UndoAction | null = this.state.undo;
    if (!u || this.state.busy) return false;
    this.set({ busy: true, error: null });
    try {
      if (u.kind === "merge") await this.b.unmergePerson(u.absorbed);
      else if (u.kind === "protect") await this.b.protectPerson(u.id, u.prev);
      else await this.b.namePerson(u.id, u.prev);
      const people = await this.b.listPeople();
      const focusId = u.kind === "merge" ? u.absorbed : u.id;
      this.set({ busy: false, undo: null, people, focusId });
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    }
  }
}
