// Ren avledning: grupper fra kort (kjernen gir `group` per kort).
import type { Card, Group } from "./types";

export function groupCards(cards: readonly Card[]): Group[] {
  const map = new Map<number, Group>();
  for (const c of cards) {
    if (c.group == null) continue;
    const g = map.get(c.group) ?? { id: c.group, keeper: c.keeper, cards: [], bytes: 0 };
    g.cards.push(c);
    g.bytes += c.bytes;
    map.set(c.group, g);
  }
  return [...map.values()];
}
