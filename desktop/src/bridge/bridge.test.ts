import { describe, expect, it, vi } from "vitest";
import type { CoreBridge } from "./core-bridge";
import { MockBridge } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";
import { groupCards } from "./groups";

// Type-test: begge klassene må være gjensidig tilordnebare med CoreBridge og ha samme nøkler.
type Keys<T> = keyof T;
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
const _k1: Same<Keys<MockBridge>, Keys<TauriBridge>> = true;
const _m: CoreBridge = new MockBridge();
const _t: CoreBridge = new TauriBridge(async () => undefined as never);
void _k1; void _m; void _t;

describe("identisk grensesnitt", () => {
  it("samme offentlige metoder", () => {
    const names = (o: object) =>
      Object.getOwnPropertyNames(Object.getPrototypeOf(o)).filter((n) => n !== "constructor" && !n.startsWith("wait")).sort();
    const pub = ["decide", "emptyTrash", "getState", "listCards", "purge", "setKeeper", "startScan", "undo"];
    expect(names(new TauriBridge())).toEqual(pub);
    expect(names(new MockBridge()).filter((n) => pub.includes(n))).toEqual(pub);
  });
});

describe("TauriBridge", () => {
  it("kaller eksisterende kommandoer med riktige argumenter", async () => {
    const call = vi.fn().mockResolvedValue(7);
    const b = new TauriBridge(call);
    await b.startScan(true);
    await b.listCards("blurry", 20, 3);
    await b.decide([1, 2], "trash");
    await b.undo();
    await b.getState();
    expect(call.mock.calls).toEqual([
      ["start_scan", { full: true }],
      ["list_cards", { kind: "blurry", limit: 20, cursor: 3 }],
      ["decide_cards", { ids: [1, 2], decision: "trash" }],
      ["undo"],
      ["get_state"],
    ]);
  });
  it("setKeeper kaller set_keeper", async () => {
    const call = vi.fn().mockResolvedValue(undefined);
    await new TauriBridge(call).setKeeper(4, 9);
    expect(call.mock.calls).toEqual([["set_keeper", { group: 4, keeper: 9 }]]);
  });
  it("purge tømmer via empty_trash og gir antall", async () => {
    const call = vi.fn().mockResolvedValue({ purged: 3, keptTrashed: 0 });
    expect(await new TauriBridge(call).purge()).toBe(3);
    expect(call.mock.calls).toEqual([["empty_trash"]]);
  });
});

describe("MockBridge", () => {
  it("er deterministisk", async () => {
    const a = await new MockBridge().listCards("exact_dup", 50);
    const b = await new MockBridge().listCards("exact_dup", 50);
    expect(a).toEqual(b);
  });
  it("skann → papirkurv → angre → purge", async () => {
    const m = new MockBridge();
    expect((await m.getState()).reclaimableBytes).toBe(0);
    await m.startScan(false);
    const s0 = await m.getState();
    expect(s0.scanPhase).toBe("done");
    const page = await m.listCards("blurry", 2);
    expect(page.cards).toHaveLength(2);
    expect(page.next).not.toBeNull();
    const ids = page.cards.map((c) => c.id);
    await m.decide(ids, "trash");
    expect((await m.getState()).trashCount).toBe(2);
    await m.undo();
    const s2 = await m.getState();
    expect(s2.trashCount).toBe(0);
    expect(s2.reclaimableBytes).toBe(s0.reclaimableBytes);
    await m.decide(ids, "trash");
    expect(await m.purge()).toBe(2);
    expect((await m.getState()).freedBytes).toBeGreaterThan(0);
  });
  it("feil: ukjent kort og ingenting å angre", async () => {
    const m = new MockBridge();
    await expect(m.decide([9999], "trash")).rejects.toMatchObject({ code: "unknown_card" });
    await expect(m.undo()).rejects.toMatchObject({ code: "nothing_to_undo" });
  });
  it("forsinkelse kan slås på", async () => {
    vi.useFakeTimers();
    const p = new MockBridge({ delayMs: 100 }).getState();
    let done = false;
    void p.then(() => (done = true));
    await vi.advanceTimersByTimeAsync(99);
    expect(done).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(done).toBe(true);
    vi.useRealTimers();
  });
  it("grupperer kort", async () => {
    const m = new MockBridge();
    const g = groupCards((await m.listCards("exact_dup", 50)).cards);
    expect(g).toHaveLength(3);
    expect(g[0].cards).toHaveLength(2);
  });
});
