// Gruppevisning: ViewState + ren reduce (PLAN-UX-I18N 3.4 og 4). Keeper er et asset utenfor forslagene (kjernen
// foreslår bare det som kan flyttes), så den vises først og er aldri med i trash-kall. Beskyttede forslag flyttes aldri.
import type { CoreBridge } from "../bridge/core-bridge";
import { groupCards } from "../bridge/groups";
import type { BridgeError, Card, Group } from "../bridge/types";

export type GroupPhase = "loading" | "ready" | "working" | "committed" | "error";

export interface GroupState {
  phase: GroupPhase;
  group: Group | null;
  /** Antall flyttet / batch som kan angres, satt i committed. */
  moved: number;
  batch: number | null;
  error: BridgeError | null;
}

export type GroupEvent =
  | { type: "loaded"; group: Group }
  | { type: "failed"; error: BridgeError }
  | { type: "start" }
  | { type: "keeperSet"; group: Group }
  | { type: "committed"; batch: number; moved: number }
  | { type: "actionFailed"; error: BridgeError }
  | { type: "reload" };

export const initialGroup: GroupState = { phase: "loading", group: null, moved: 0, batch: null, error: null };

export function reduceGroup(s: GroupState, e: GroupEvent): GroupState {
  switch (e.type) {
    case "loaded":
      return s.phase === "loading" ? { ...s, phase: "ready", group: e.group, error: null } : s;
    case "failed":
      return s.phase === "loading" ? { ...s, phase: "error", error: e.error } : s;
    case "start":
      return s.phase === "ready" ? { ...s, phase: "working", error: null } : s;
    case "keeperSet":
      return s.phase === "working" ? { ...s, phase: "ready", group: e.group } : s;
    case "committed":
      return s.phase === "working" ? { ...s, phase: "committed", batch: e.batch, moved: e.moved } : s;
    case "actionFailed":
      return s.phase === "working" ? { ...s, phase: "ready", error: e.error } : s;
    case "reload":
      return { ...initialGroup };
  }
}

export interface GroupItem {
  /** 1-basert posisjon, også tastenummer (1–9). */
  n: number;
  asset: number;
  /** Kort-id, null for keeper. */
  cardId: number | null;
  best: boolean;
  protected: boolean;
  /** Ukjent for keeper (kortet bærer bare forslagets bytes). */
  bytes: number | null;
}

/** Keeper først (forhåndsmerket «Beste»), så forslagene. Tom liste hvis gruppen mangler keeper. */
export function groupItems(g: Group): GroupItem[] {
  const items: GroupItem[] = [];
  if (g.keeper !== null) items.push({ n: 1, asset: g.keeper, cardId: null, best: true, protected: false, bytes: null });
  for (const c of g.cards) {
    items.push({ n: items.length + 1, asset: c.asset, cardId: c.id, best: false, protected: c.protectFlags !== 0, bytes: c.bytes });
  }
  return items;
}

/** Kort som «Behold dette» flytter: aldri beskyttede, aldri keeper (keeper er ikke et kort). */
export const movableCards = (g: Group): Card[] => g.cards.filter((c) => c.protectFlags === 0);

/** Tastenummer -> asset som skal bli ny keeper, eller null (utenfor område, eller allerede beste). */
export function keeperForKey(g: Group, key: string): number | null {
  if (!/^[1-9]$/.test(key)) return null;
  const item = groupItems(g)[Number(key) - 1];
  return item && !item.best ? item.asset : null;
}

/** Finner gruppen ved å bla duplikatkategoriene (rute #/group/:id bærer ikke kategori). null = finnes ikke (lenger). */
export async function findGroup(bridge: CoreBridge, id: number): Promise<Group | null> {
  for (const kind of ["exact_dup", "visual_dup"] as const) {
    const cards: Card[] = [];
    let cursor: number | null = null;
    do {
      const page = await bridge.listCards(kind, 200, cursor);
      cards.push(...page.cards);
      cursor = page.next;
    } while (cursor !== null);
    const g = groupCards(cards).find((x) => x.id === id);
    if (g) return g;
  }
  return null;
}
