// Gjennomgang: ViewState + ren reduce (PLAN-UX-I18N 3.3 og 4). Ingen I/O: kalleren gjør bridge-kall og sender resultat som events.
// Beskyttede kort er aldri valgbare (aldri i «Velg alle»); sum og bytes summeres her fra kortdata, ikke via kjernekall.
import type { BridgeError, Card } from "../bridge/types";

/** Fra og med så mange bilder kreves bekreftelsesark med avkrysning (MassAck). */
export const MASS_ACK = 50;
/** Kort per side (markørpaginering). */
export const PAGE_SIZE = 60;

export type ReviewPhase = "loading" | "list" | "committing" | "committed" | "empty" | "error";

export interface ReviewState {
  phase: ReviewPhase;
  items: readonly Card[];
  /** Markør til neste side, null = ingen flere. */
  next: number | null;
  selected: ReadonlySet<number>;
  /** Sist klikkede kort (ankerpunkt for Shift+klikk). */
  anchor: number | null;
  confirming: boolean;
  ack: boolean;
  /** Batch som kan angres, og hvor mange den flyttet. */
  batch: number | null;
  moved: number;
  error: BridgeError | null;
}

export type ReviewEvent =
  | { type: "loaded"; cards: readonly Card[]; next: number | null; append?: boolean }
  | { type: "failed"; error: BridgeError }
  | { type: "toggle"; id: number }
  | { type: "toggleMany"; ids: readonly number[] }
  | { type: "range"; id: number; order: readonly number[] }
  | { type: "selectAll" }
  | { type: "clear" }
  | { type: "requestCommit" }
  | { type: "cancelConfirm" }
  | { type: "setAck"; value: boolean }
  | { type: "confirmCommit" }
  | { type: "committed"; batch: number }
  | { type: "commitFailed"; error: BridgeError }
  | { type: "reload" };

export const initialReview: ReviewState = {
  phase: "loading", items: [], next: null, selected: new Set(), anchor: null,
  confirming: false, ack: false, batch: null, moved: 0, error: null,
};

/** Liste som kan brukes (også rett etter commit, med Angre synlig). */
const isLive = (s: ReviewState): boolean => s.phase === "list" || s.phase === "committed";

export const isProtected = (c: Card): boolean => c.protectFlags !== 0;
export const selectable = (c: Card): boolean => !isProtected(c);

export function summarize(items: readonly Card[], selected: ReadonlySet<number>): { count: number; bytes: number } {
  let count = 0;
  let bytes = 0;
  for (const c of items) {
    if (selected.has(c.id)) { count++; bytes += c.bytes; }
  }
  return { count, bytes };
}

export const needsAck = (count: number): boolean => count >= MASS_ACK;

const byId = (items: readonly Card[]) => new Map(items.map((c) => [c.id, c]));

/** Velger kun kort som finnes og er valgbare. */
function only(items: readonly Card[], ids: Iterable<number>): Set<number> {
  const m = byId(items);
  const out = new Set<number>();
  for (const id of ids) {
    const c = m.get(id);
    if (c && selectable(c)) out.add(id);
  }
  return out;
}

export function reduceReview(s: ReviewState, e: ReviewEvent): ReviewState {
  switch (e.type) {
    case "loaded": {
      if (s.phase !== "loading" && !isLive(s)) return s;
      const items = e.append ? [...s.items, ...e.cards] : e.cards;
      return {
        ...s, items, next: e.next, phase: items.length === 0 ? "empty" : s.phase === "committed" ? "committed" : "list",
        selected: e.append ? s.selected : new Set(), anchor: e.append ? s.anchor : null, error: null,
      };
    }
    case "failed":
      return s.phase === "loading" || isLive(s) ? { ...s, phase: "error", error: e.error } : s;
    case "toggle": {
      if (!isLive(s)) return s;
      const next = new Set(s.selected);
      if (!only(s.items, [e.id]).size) return s;
      if (!next.delete(e.id)) next.add(e.id);
      return { ...s, selected: next, anchor: e.id };
    }
    case "toggleMany": {
      if (!isLive(s)) return s;
      const ids = only(s.items, e.ids);
      if (!ids.size) return s;
      const allOn = [...ids].every((id) => s.selected.has(id));
      const next = new Set(s.selected);
      for (const id of ids) allOn ? next.delete(id) : next.add(id);
      return { ...s, selected: next };
    }
    case "range": {
      if (!isLive(s)) return s;
      const a = s.anchor === null ? -1 : e.order.indexOf(s.anchor);
      const b = e.order.indexOf(e.id);
      if (a < 0 || b < 0) return reduceReview(s, { type: "toggle", id: e.id });
      const [lo, hi] = a < b ? [a, b] : [b, a];
      const next = new Set(s.selected);
      for (const id of only(s.items, e.order.slice(lo, hi + 1))) next.add(id);
      return { ...s, selected: next, anchor: e.id };
    }
    case "selectAll":
      return isLive(s) ? { ...s, selected: only(s.items, s.items.map((c) => c.id)) } : s;
    case "clear":
      return isLive(s) ? { ...s, selected: new Set(), anchor: null } : s;
    case "requestCommit": {
      if (!isLive(s)) return s;
      const { count } = summarize(s.items, s.selected);
      if (count === 0) return s; // aldri tom handling
      return needsAck(count)
        ? { ...s, confirming: true, ack: false }
        : { ...s, phase: "committing", error: null, batch: null };
    }
    case "cancelConfirm":
      return s.confirming ? { ...s, confirming: false, ack: false } : s;
    case "setAck":
      return s.confirming ? { ...s, ack: e.value } : s;
    case "confirmCommit":
      return isLive(s) && s.confirming && s.ack
        ? { ...s, phase: "committing", confirming: false, ack: false, error: null, batch: null }
        : s;
    case "committed": {
      if (s.phase !== "committing") return s;
      const moved = summarize(s.items, s.selected).count;
      const items = s.items.filter((c) => !s.selected.has(c.id));
      return { ...s, phase: "committed", items, selected: new Set(), anchor: null, batch: e.batch, moved };
    }
    case "commitFailed":
      return s.phase === "committing" ? { ...s, phase: "list", error: e.error } : s;
    case "reload":
      return { ...initialReview };
  }
}

/** ReasonCode (PascalCase fra kjernen) -> nøkkelsuffiks i `reason.*` (snake_case). */
export const reasonSnake = (code: string): string => code.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
