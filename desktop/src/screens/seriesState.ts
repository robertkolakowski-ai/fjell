// Serie-sammenligning: ViewState + ren reduce, og avledninger (medlemmer side om side, forhåndsmerking, zoom).
// Behold-kandidaten (rang 1) er et bilde utenfor forslagene og flyttes aldri. Alt annet forhåndsmerkes for papirkurv,
// unntatt beskyttede bilder: de er aldri forhåndsmerket og kan ikke merkes. Appen foreslår; brukeren bestemmer.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError, Card, Detail, ReasonCode } from "../bridge/types";
import type { SeriesCompare, SeriesMember } from "../bridge/types-v1";

export type SeriesPhase = "loading" | "ready" | "working" | "committed" | "error";

export interface SeriesState {
  phase: SeriesPhase;
  compare: SeriesCompare | null;
  /** Kort-id per asset (bare bilder som er forslag har kort; behold-kandidaten har ikke). */
  cardOf: ReadonlyMap<number, number>;
  /** Assets merket for papirkurv. */
  marked: ReadonlySet<number>;
  /** Felles zoom for alle bildene (1 = tilpasset). */
  zoom: number;
  /** Resultat av siste valg som kan angres: hva som ble gjort og hvor mange. */
  done: "trash" | "keep" | null;
  moved: number;
  /** Kjernen ba om bekreftelse for beskyttede bilder; ventende «flytt» står klar. */
  askProtected: boolean;
  error: BridgeError | null;
}

export type SeriesEvent =
  | { type: "loaded"; compare: SeriesCompare; cards: readonly Card[] }
  | { type: "failed"; error: BridgeError }
  | { type: "toggleMark"; asset: number }
  | { type: "start" }
  | { type: "refreshed"; compare: SeriesCompare; cards: readonly Card[] }
  | { type: "committed"; done: "trash" | "keep"; moved: number }
  | { type: "actionFailed"; error: BridgeError }
  | { type: "askProtected" }
  | { type: "dismissAsk" }
  | { type: "zoomIn" }
  | { type: "zoomOut" }
  | { type: "zoomReset" }
  | { type: "reload" };

/** Zoomtrinn (felles for alle bildene). */
export const ZOOM_STEPS = [1, 1.5, 2, 3, 4] as const;

export const initialSeries: SeriesState = {
  phase: "loading", compare: null, cardOf: new Map(), marked: new Set(), zoom: 1, done: null, moved: 0, askProtected: false, error: null,
};

const cardMap = (cards: readonly Card[]): Map<number, number> => new Map(cards.map((c) => [c.asset, c.id] as const));

/** Kan et medlem merkes for papirkurv? Ikke behold-kandidaten, ikke beskyttede, og bare med kort (forslag). */
export const markable = (m: SeriesMember, cardOf: ReadonlyMap<number, number>): boolean =>
  !m.isKeeper && m.protectFlags === 0 && cardOf.has(m.asset);

/** Forhåndsmerking: alt unntatt behold-kandidaten og beskyttede bilder. */
export function premarked(c: SeriesCompare, cardOf: ReadonlyMap<number, number>): Set<number> {
  return new Set(c.members.filter((m) => markable(m, cardOf)).map((m) => m.asset));
}

const stepIndex = (z: number): number => {
  let best = 0;
  ZOOM_STEPS.forEach((s, i) => { if (Math.abs(s - z) < Math.abs(ZOOM_STEPS[best] - z)) best = i; });
  return best;
};

export function reduceSeries(s: SeriesState, e: SeriesEvent): SeriesState {
  switch (e.type) {
    case "reload":
      return { ...initialSeries };
    case "loaded": {
      if (s.phase !== "loading") return s;
      const cardOf = cardMap(e.cards);
      return { ...s, phase: "ready", compare: e.compare, cardOf, marked: premarked(e.compare, cardOf), error: null };
    }
    case "failed":
      return s.phase === "loading" ? { ...s, phase: "error", error: e.error } : s;
    case "toggleMark": {
      if (s.phase !== "ready" || !s.compare) return s;
      const m = s.compare.members.find((x) => x.asset === e.asset);
      if (!m || !markable(m, s.cardOf)) return s;
      const marked = new Set(s.marked);
      if (!marked.delete(e.asset)) marked.add(e.asset);
      return { ...s, marked };
    }
    case "start":
      return s.phase === "ready" ? { ...s, phase: "working", error: null } : s;
    case "refreshed": {
      // Ny behold-kandidat eller endret beskyttelse: forhåndsmerkingen regnes på nytt fra kjernens svar.
      if (s.phase !== "working") return s;
      const cardOf = cardMap(e.cards);
      return { ...s, phase: "ready", compare: e.compare, cardOf, marked: premarked(e.compare, cardOf), askProtected: false };
    }
    case "committed":
      return s.phase === "working" ? { ...s, phase: "committed", done: e.done, moved: e.moved, askProtected: false } : s;
    case "actionFailed":
      return s.phase === "working" ? { ...s, phase: "ready", error: e.error } : s;
    case "askProtected":
      return s.phase === "working" ? { ...s, phase: "ready", askProtected: true, error: null } : s;
    case "dismissAsk":
      return s.askProtected ? { ...s, askProtected: false } : s;
    case "zoomIn":
      return { ...s, zoom: ZOOM_STEPS[Math.min(ZOOM_STEPS.length - 1, stepIndex(s.zoom) + 1)] };
    case "zoomOut":
      return { ...s, zoom: ZOOM_STEPS[Math.max(0, stepIndex(s.zoom) - 1)] };
    case "zoomReset":
      return { ...s, zoom: 1 };
  }
}

/** Ett medlem slik skjermen viser det. */
export interface SeriesItem {
  /** 1-basert plass i visningen, også tastenummer (1–9). */
  n: number;
  asset: number;
  rank: number;
  score: number;
  best: boolean;
  protected: boolean;
  /** Kan merkes for papirkurv. */
  markable: boolean;
  marked: boolean;
  closedEyes: boolean;
  bytes: number;
  reason: ReasonCode;
  detail: Detail;
}

/** Medlemmene i visningsrekkefølge (behold-kandidaten først, så etter rangering). */
export function seriesItems(s: SeriesState): SeriesItem[] {
  if (!s.compare) return [];
  return [...s.compare.members].sort((a, b) => a.rank - b.rank).map((m, i) => ({
    n: i + 1,
    asset: m.asset,
    rank: m.rank,
    score: m.score,
    best: m.isKeeper,
    protected: m.protectFlags !== 0,
    markable: markable(m, s.cardOf),
    marked: s.marked.has(m.asset),
    closedEyes: m.reason === "ClosedEyes",
    bytes: m.bytes,
    reason: m.reason as ReasonCode,
    detail: m.detail,
  }));
}

/** Kort-id-er som «Flytt» sender til papirkurven: merkede, ikke beskyttede, ikke behold-kandidaten. */
export function moveIds(s: SeriesState): number[] {
  if (!s.compare) return [];
  return s.compare.members.filter((m) => markable(m, s.cardOf) && s.marked.has(m.asset)).map((m) => s.cardOf.get(m.asset)!);
}

/** Alle kort i serien (til «Behold alle»). */
export const allCardIds = (s: SeriesState): number[] => [...s.cardOf.values()];

/** Bytes for det som er merket (til annonsering og knapp). */
export function markedBytes(s: SeriesState): number {
  return seriesItems(s).filter((i) => i.markable && i.marked).reduce((n, i) => n + i.bytes, 0);
}

/** Tastenummer 1–9 -> asset som skal bli ny behold-kandidat, eller null (utenfor område eller allerede beste). */
export function keeperForKey(items: readonly SeriesItem[], key: string): number | null {
  if (!/^[1-9]$/.test(key)) return null;
  const it = items[Number(key) - 1];
  return it && !it.best ? it.asset : null;
}

/** Zoomtast (+, −, 0) -> hendelse. Ctrl/Alt/Meta gir aldri en tast. */
export function zoomKey(e: { key: string; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean }): "zoomIn" | "zoomOut" | "zoomReset" | null {
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
  if (e.key === "+" || e.key === "=") return "zoomIn";
  if (e.key === "-" || e.key === "−") return "zoomOut";
  if (e.key === "0") return "zoomReset";
  return null;
}

/** Medlemmer og kort for én serie. Kortene hentes fra serieforslagene (rute #/series/:id bærer ikke annet enn serie-id). */
export async function loadSeries(b: CoreBridge, id: number): Promise<{ compare: SeriesCompare; cards: Card[] }> {
  const compare = await b.compareSeries(id);
  const cards: Card[] = [];
  let cursor: number | null = null;
  do {
    const page = await b.listCards("series", 200, cursor);
    cards.push(...page.cards.filter((c) => c.group === id));
    cursor = page.next;
  } while (cursor !== null);
  return { compare, cards };
}
