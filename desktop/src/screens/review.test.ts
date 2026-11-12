import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { Card } from "../bridge/types";
import { initialReview, MASS_ACK, reduceReview, summarize, type ReviewEvent, type ReviewState } from "./reviewState";

const card = (id: number, over: Partial<Card> = {}): Card => ({
  id, kind: "blurry", asset: id, group: null, keeper: null, reason: "Blurry", confidence: 0.9, bytes: 100 * id, protectFlags: 0, ...over,
});
const run = (s: ReviewState, ...es: ReviewEvent[]) => es.reduce(reduceReview, s);
const loaded = (cards: Card[]) => run(initialReview, { type: "loaded", cards, next: null });

describe("gjennomgang: valg og sum", () => {
  it("summerer antall og bytes fra kortdata", () => {
    const s = run(loaded([card(1), card(2), card(3)]), { type: "toggle", id: 1 }, { type: "toggle", id: 3 });
    expect(summarize(s.items, s.selected)).toEqual({ count: 2, bytes: 400 });
  });
  it("Velg alle tar aldri beskyttede, og beskyttede kan ikke velges enkeltvis", () => {
    const s = run(loaded([card(1), card(2, { protectFlags: 1 })]), { type: "selectAll" }, { type: "toggle", id: 2 });
    expect([...s.selected]).toEqual([1]);
  });
  it("Shift+klikk velger område, rekkefølge uavhengig av retning", () => {
    const order = [1, 2, 3, 4, 5];
    const s = run(loaded(order.map((i) => card(i))), { type: "toggle", id: 4 }, { type: "range", id: 2, order });
    expect([...s.selected].sort()).toEqual([2, 3, 4]);
  });
  it("tom handling gir ingenting", () => {
    const s = run(loaded([card(1)]), { type: "requestCommit" });
    expect(s.phase).toBe("list");
    expect(s.confirming).toBe(false);
  });
  it("tom kategori er empty", () => {
    expect(loaded([]).phase).toBe("empty");
  });
});

describe("gjennomgang: commit", () => {
  it("under terskel committer direkte, ved terskel kreves avkrysning", () => {
    const few = run(loaded([card(1)]), { type: "toggle", id: 1 }, { type: "requestCommit" });
    expect(few.phase).toBe("committing");

    const many = Array.from({ length: MASS_ACK }, (_, i) => card(i + 1));
    const s = run(loaded(many), { type: "selectAll" }, { type: "requestCommit" });
    expect(s).toMatchObject({ phase: "list", confirming: true });
    expect(run(s, { type: "confirmCommit" }).phase).toBe("list"); // uten avkrysning
    const ok = run(s, { type: "setAck", value: true }, { type: "confirmCommit" });
    expect(ok.phase).toBe("committing");
  });
  it("avbryt i arket nullstiller avkrysning", () => {
    const many = Array.from({ length: MASS_ACK }, (_, i) => card(i + 1));
    const s = run(loaded(many), { type: "selectAll" }, { type: "requestCommit" }, { type: "setAck", value: true }, { type: "cancelConfirm" });
    expect(s).toMatchObject({ confirming: false, ack: false, phase: "list" });
  });
  it("feil ruller tilbake: valget består", () => {
    const s = run(loaded([card(1)]), { type: "toggle", id: 1 }, { type: "requestCommit" }, { type: "commitFailed", error: { code: "core_error", detail: "" } });
    expect(s.phase).toBe("list");
    expect(s.selected.size).toBe(1);
    expect(s.items).toHaveLength(1);
  });
});

describe("gjennomgang mot MockBridge", () => {
  it("én batch flytter valgte, angre gjenoppretter", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const page = await b.listCards("blurry", 60);
    let s = run(initialReview, { type: "loaded", cards: page.cards, next: page.next }, { type: "selectAll" }, { type: "requestCommit" });
    expect(s.phase).toBe("committing");
    const ids = s.items.filter((c) => s.selected.has(c.id)).map((c) => c.id);
    s = reduceReview(s, { type: "committed", batch: await b.decide(ids, "trash") });
    expect(s).toMatchObject({ phase: "committed", moved: 6, items: [] });
    expect((await b.listCards("blurry", 60)).cards).toHaveLength(0);
    expect((await b.getState()).trashCount).toBe(6);
    await b.undo();
    expect((await b.listCards("blurry", 60)).cards).toHaveLength(6);
  });
});
