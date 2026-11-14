// Personer: ren tilstand og hjelpefunksjoner (ingen bridge her). Flyten ligger i peopleFlow.ts.
import type { Person } from "../bridge/types-v1";

/** Størst tillatte lengde på et personnavn (tegn). Samme grense som kjernen (`name_too_long`). */
export const PERSON_NAME_MAX = 100;

/** Siste handling som kan angres. Vises som «Angre» i Toast. */
export type UndoAction =
  | { kind: "merge"; absorbed: number; target: number; absorbedLabel: string; targetLabel: string }
  | { kind: "protect"; id: number; prev: boolean; label: string }
  | { kind: "name"; id: number; prev: string | null; label: string };

export interface MergeDraft {
  /** Personen brukeren startet fra. */
  from: number;
  /** Den andre personen (valgt i dialogen), eller null før valg. */
  other: number | null;
  /** Hvem som blir stående: den man startet fra eller den andre. */
  keep: "from" | "other";
}

export interface PeopleState {
  phase: "loading" | "ready" | "error";
  faceIndex: boolean;
  people: Person[];
  /** Personen som har åpent navnefelt. */
  editing: number | null;
  /** Feilkode for navnefeltet (vises under feltet). */
  nameError: string | null;
  merge: MergeDraft | null;
  consent: boolean;
  busy: boolean;
  /** Siste feil fra en handling (kode). */
  error: string | null;
  undo: UndoAction | null;
  /** Kortvarig melding til skjermleser (aria-live). */
  announce: string;
  /** Personen som skal få fokus etter en endring i listen. */
  focusId: number | null;
}

export const initialPeople: PeopleState = {
  phase: "loading", faceIndex: false, people: [], editing: null, nameError: null, merge: null, consent: false,
  busy: false, error: null, undo: null, announce: "", focusId: null,
};

/** Visningsnavn: eget navn eller «Uten navn» (teksten kommer fra i18n). */
export function displayName(p: Person, unnamed: string): string {
  return p.name && p.name.trim() !== "" ? p.name : unnamed;
}

/** Den som er best å beholde ved sammenslåing: den med navn, ellers den med flest ansikter. */
export function defaultKeep(from: Person, other: Person): "from" | "other" {
  const a = !!from.name, b = !!other.name;
  if (a !== b) return a ? "from" : "other";
  return other.faceCount > from.faceCount ? "other" : "from";
}

/** Utfallet av en sammenslåing: hvem som slås inn i hvem. */
export function mergeRoles(draft: MergeDraft): { target: number; absorbed: number } | null {
  if (draft.other === null) return null;
  return draft.keep === "from"
    ? { target: draft.from, absorbed: draft.other }
    : { target: draft.other, absorbed: draft.from };
}

/** Feilkode for et navn før det sendes, eller null. Tomt navn er lov (fjerner navnet). */
export function validateName(raw: string): string | null {
  return [...raw.trim()].length > PERSON_NAME_MAX ? "name_too_long" : null;
}
