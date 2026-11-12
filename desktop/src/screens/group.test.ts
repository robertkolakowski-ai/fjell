import { describe, expect, it } from "vitest";
import { groupCards } from "../bridge/groups";
import { MockBridge } from "../bridge/mock-bridge";
import type { Card, Group } from "../bridge/types";
import { findGroup, groupItems, initialGroup, keeperForKey, movableCards, reduceGroup } from "./groupState";

const mk = (id: number, over: Partial<Card> = {}): Card => ({
  id, kind: "exact_dup", asset: id, group: 1, keeper: 900, reason: "ExactCopy", confidence: 1, bytes: 10, protectFlags: 0, ...over,
});
const grp = (cards: Card[]): Group => groupCards(cards)[0];

describe("gruppe: rene funksjoner", () => {
  it("keeper først, forhåndsmerket Beste, deretter forslag", () => {
    const items = groupItems(grp([mk(1), mk(2)]));
    expect(items.map((i) => [i.n, i.asset, i.best])).toEqual([[1, 900, true], [2, 1, false], [3, 2, false]]);
  });
  it("beskyttede flyttes aldri, keeper er aldri et kort", () => {
    const g = grp([mk(1), mk(2, { protectFlags: 2 })]);
    expect(movableCards(g).map((c) => c.id)).toEqual([1]);
    expect(movableCards(g).some((c) => c.asset === g.keeper)).toBe(false);
  });
  it("tast 1–9: nåværende beste og utenfor område gir null", () => {
    const g = grp([mk(1), mk(2)]);
    expect(keeperForKey(g, "1")).toBeNull();
    expect(keeperForKey(g, "3")).toBe(2);
    expect(keeperForKey(g, "9")).toBeNull();
    expect(keeperForKey(g, "0")).toBeNull();
    expect(keeperForKey(g, "a")).toBeNull();
  });
  it("reduce: handling kun fra ready", () => {
    const g = grp([mk(1)]);
    const r = reduceGroup(initialGroup, { type: "loaded", group: g });
    expect(reduceGroup(initialGroup, { type: "start" })).toBe(initialGroup);
    const w = reduceGroup(r, { type: "start" });
    expect(w.phase).toBe("working");
    expect(reduceGroup(w, { type: "actionFailed", error: { code: "x", detail: "" } }).phase).toBe("ready");
    expect(reduceGroup(w, { type: "committed", batch: 1, moved: 1 })).toMatchObject({ phase: "committed", moved: 1 });
  });
});

describe("gruppe mot MockBridge", () => {
  it("finner gruppe, bytter keeper, flytter resten som én batch, angrer", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const g = (await findGroup(b, 1))!;
    expect(g.cards).toHaveLength(2);
    const oldKeeper = g.keeper!;
    const pick = g.cards[0];
    expect(keeperForKey(g, "2")).toBe(pick.asset);

    await b.setKeeper(1, pick.asset);
    const g2 = (await findGroup(b, 1))!;
    expect(g2.keeper).toBe(pick.asset);
    expect(g2.cards.map((c) => c.asset)).toContain(oldKeeper);
    expect(g2.cards.map((c) => c.asset)).not.toContain(pick.asset);

    const ids = movableCards(g2).map((c) => c.id);
    await b.decide(ids, "trash");
    expect(await findGroup(b, 1)).toBeNull();
    await b.undo();
    expect((await findGroup(b, 1))!.cards).toHaveLength(2);
  });
  it("set_keeper avviser fremmed asset", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    await expect(b.setKeeper(1, 424242)).rejects.toMatchObject({ code: "unknown_asset" });
  });
});
