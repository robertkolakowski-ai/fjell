// Kortmodus: ViewState + ren reduce (PLAN-UX-I18N 3.5 og 4), tastetabell (6.4) og dra-avgjørelse. Ingen I/O.
// Kalleren gjør bridge-kall (seriell kø) og sender utfall som events. Beslutninger er optimistiske: kortet
// går av bunken umiddelbart (< 100 ms), og rulles tilbake hvis kjernen avviser.
import type { BridgeError, Card, Decision } from "../bridge/types";
import { isProtected } from "./reviewState";

export type CardsPhase = "loading" | "card" | "deckEmpty" | "error";
export type CardAction = "keep" | "trash" | "skip";
/** Hva som skal annonseres i live-regionen (teksten bygges i UI med antall igjen). */
export type LiveKind = "trashed" | "kept" | "skipped" | "undone";

export interface HistoryEntry {
  card: Card;
  decision: Decision;
}

export interface CardsState {
  phase: CardsPhase;
  /** Bunken: første = øverste kort. «Hopp over» flytter øverste bakerst. */
  queue: readonly Card[];
  /** Angre-stabel (nyeste sist). Én oppføring per kjernebatch. */
  history: readonly HistoryEntry[];
  trashed: number;
  kept: number;
  zoom: boolean;
  live: { kind: LiveKind; seq: number } | null;
  error: BridgeError | null;
}

export type CardsEvent =
  | { type: "loaded"; cards: readonly Card[] }
  | { type: "failed"; error: BridgeError }
  | { type: "act"; action: CardAction }
  /** Kjernen avviste beslutningen: kortet tilbake øverst. */
  | { type: "rollback"; card: Card; error: BridgeError }
  | { type: "undo" }
  | { type: "toggleZoom" }
  | { type: "closeZoom" }
  | { type: "reload" };

export const initialCards: CardsState = {
  phase: "loading", queue: [], history: [], trashed: 0, kept: 0, zoom: false, live: null, error: null,
};

/** Kort som kan foreslås i kortmodus: beskyttede bilder (keeper, beskyttet) kan aldri flyttes til papirkurv. */
export const deckOf = (cards: readonly Card[]): Card[] => cards.filter((c) => !isProtected(c));

const say = (s: CardsState, kind: LiveKind): CardsState["live"] => ({ kind, seq: (s.live?.seq ?? 0) + 1 });
const settle = (s: CardsState): CardsState => (s.queue.length === 0 ? { ...s, phase: "deckEmpty", zoom: false } : { ...s, phase: "card" });

export function reduceCards(s: CardsState, e: CardsEvent): CardsState {
  switch (e.type) {
    case "reload":
      return { ...initialCards };
    case "loaded":
      return settle({ ...initialCards, queue: deckOf(e.cards) });
    case "failed":
      return { ...s, phase: "error", error: e.error };
    case "toggleZoom":
      return s.phase === "card" ? { ...s, zoom: !s.zoom } : s;
    case "closeZoom":
      return s.zoom ? { ...s, zoom: false } : s;
    case "act": {
      if (s.phase !== "card") return s;
      const [top, ...rest] = s.queue;
      if (e.action === "skip") {
        // Ingen kjernekall. Med bare ett kort igjen finnes det ingenting å hoppe til.
        if (rest.length === 0) return s;
        return { ...s, queue: [...rest, top], zoom: false, live: say(s, "skipped") };
      }
      const decision: Decision = e.action;
      return settle({
        ...s,
        queue: rest,
        history: [...s.history, { card: top, decision }],
        trashed: s.trashed + (decision === "trash" ? 1 : 0),
        kept: s.kept + (decision === "keep" ? 1 : 0),
        zoom: false,
        error: null,
        live: say(s, decision === "trash" ? "trashed" : "kept"),
      });
    }
    case "rollback": {
      const i = s.history.map((h) => h.card.id).lastIndexOf(e.card.id);
      if (i < 0) return { ...s, error: e.error };
      const h = s.history[i];
      return {
        ...s, phase: "card", queue: [e.card, ...s.queue], error: e.error,
        history: s.history.filter((_, j) => j !== i),
        trashed: s.trashed - (h.decision === "trash" ? 1 : 0),
        kept: s.kept - (h.decision === "keep" ? 1 : 0),
      };
    }
    case "undo": {
      const last = s.history[s.history.length - 1];
      if (!last || (s.phase !== "card" && s.phase !== "deckEmpty")) return s;
      return {
        ...s, phase: "card", queue: [last.card, ...s.queue], history: s.history.slice(0, -1), zoom: false, error: null,
        trashed: s.trashed - (last.decision === "trash" ? 1 : 0),
        kept: s.kept - (last.decision === "keep" ? 1 : 0),
        live: say(s, "undone"),
      };
    }
  }
}

/** Hurtigtaster i kortmodus (6.4). Én tabell: UI, aria-keyshortcuts og F1-hjelp leser herfra. */
export const CARD_KEYS = [
  { id: "keep", keys: ["ArrowLeft"], label: "←" },
  { id: "trash", keys: ["ArrowUp", "Delete"], label: "↑ / Delete" },
  { id: "skip", keys: ["ArrowRight"], label: "→" },
  { id: "zoom", keys: [" "], label: "space" },
  { id: "undo", keys: ["Control+Z"], label: "Ctrl+Z" },
  { id: "help", keys: ["F1"], label: "F1" },
] as const;
export type CardKeyId = (typeof CARD_KEYS)[number]["id"];
/** `aria-keyshortcuts`-verdi for en handling (mellomromsseparert, «Space» i stedet for « »). */
export const ariaKeys = (id: CardKeyId): string =>
  CARD_KEYS.find((k) => k.id === id)!.keys.map((k) => (k === " " ? "Space" : k)).join(" ");

export interface KeyLike {
  key: string;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  /** Tastetrykket treffer et tekstfelt (aldri hurtigtast der). */
  inTextField?: boolean;
  /** Fokus er på en knapp: Mellomrom skal da aktivere knappen, ikke zoome. */
  onButton?: boolean;
}

/** Tast -> handling. Ingen Alt/Win-kombinasjoner, ingen enkelttegn-taster (WCAG 2.1.4), aldri i tekstfelt. */
export function cardKey(k: KeyLike): CardKeyId | null {
  if (k.inTextField || k.altKey || k.metaKey) return null;
  if (k.ctrlKey) return k.key.toLowerCase() === "z" && !k.shiftKey ? "undo" : null;
  if (k.shiftKey) return null;
  switch (k.key) {
    case "ArrowLeft": return "keep";
    case "ArrowUp": case "Delete": return "trash";
    case "ArrowRight": return "skip";
    case " ": return k.onButton ? null : "zoom";
    case "F1": return "help";
    default: return null;
  }
}

export type DragIntent = CardAction | null;
/** Mus-dra: ≥ 35 % av bredde (venstre/høyre) eller høyde (opp), eller rask sveip. Under terskel spretter kortet tilbake. */
export function dragIntent(dx: number, dy: number, w: number, h: number, speed = 0): DragIntent {
  const fast = speed > 0.8; // px/ms
  const horiz = Math.abs(dx) >= Math.abs(dy);
  if (horiz) {
    if (Math.abs(dx) >= 0.35 * w || (fast && Math.abs(dx) > 40)) return dx < 0 ? "keep" : "skip";
    return null;
  }
  if (dy < 0 && (-dy >= 0.35 * h || (fast && -dy > 40))) return "trash";
  return null;
}
