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
    const pub = [
      "cancelScan", "completeOnboarding", "decide", "emptyTrash", "getFirstRun", "getLibrary", "getSettings", "getState",
      "listCards", "listTrashed", "pauseScan", "purge", "restore", "restoreAll", "resumeScan", "setKeeper", "setLibrary",
      "setTrashDays", "startScan", "undo",
    ];
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

describe("TauriBridge: M7-hull", () => {
  it("kaller riktige kommandoer med riktige argumenter", async () => {
    const call = vi.fn().mockResolvedValue(undefined);
    const b = new TauriBridge(call);
    await b.listTrashed();
    await b.restore([4, 5]);
    await b.restoreAll();
    await b.pauseScan();
    await b.resumeScan();
    await b.cancelScan();
    await b.getLibrary();
    await b.setLibrary("D:\\Bilder");
    await b.getFirstRun();
    await b.completeOnboarding();
    await b.getSettings();
    await b.setTrashDays(14);
    expect(call.mock.calls).toEqual([
      ["list_trashed"], ["restore_trashed", { ids: [4, 5] }], ["restore_all_trashed"], ["pause_scan"], ["resume_scan"],
      ["cancel_scan"], ["get_library"], ["set_library", { path: "D:\\Bilder" }], ["get_first_run"], ["complete_onboarding"],
      ["get_settings"], ["set_trash_days", { days: 14 }],
    ]);
  });
});

describe("MockBridge: papirkurvliste og gjenoppretting", () => {
  it("restore tar bildet ut av listen, uten å lage nytt forslag, og angre treffer ikke gjenopprettede", async () => {
    const m = new MockBridge();
    await m.startScan(false);
    const ids = (await m.listCards("blurry", 3)).cards.map((c) => c.id);
    await m.decide(ids, "trash");
    const [a, ...rest] = await m.listTrashed();
    expect(rest).toHaveLength(2);
    expect(await m.restore([a.asset, 99999])).toEqual({ restored: 1 });
    expect((await m.listTrashed()).map((i) => i.asset)).toEqual(rest.map((i) => i.asset));
    expect((await m.getState()).trashCount).toBe(2);
    expect(await m.restore([a.asset])).toEqual({ restored: 0 });
    // Angre tar de to som ligger igjen (batchen er justert), ikke det gjenopprettede.
    await m.undo();
    expect((await m.getState()).trashCount).toBe(0);
  });
  it("restoreAll tømmer listen", async () => {
    const m = new MockBridge();
    await m.startScan(false);
    await m.decide((await m.listCards("screenshot", 4)).cards.map((c) => c.id), "trash");
    expect(await m.restoreAll()).toEqual({ restored: 4 });
    expect(await m.listTrashed()).toEqual([]);
    expect(await m.restoreAll()).toEqual({ restored: 0 });
  });
  it("utløpt vises i state og liste, men bildene finnes fortsatt (aldri stille slettet)", async () => {
    let t = 0;
    const m = new MockBridge({ now: () => t, trashDays: 1 });
    await m.startScan(false);
    await m.decide([(await m.listCards("blurry", 1)).cards[0].id], "trash");
    t = 2 * 86_400_000;
    expect((await m.getState()).trashExpired).toBe(1);
    expect((await m.listTrashed())[0].expired).toBe(true);
    expect((await m.getState()).trashCount).toBe(1);
  });
  it("tøm fjerner fra listen", async () => {
    const m = new MockBridge();
    await m.startScan(false);
    await m.decide([(await m.listCards("blurry", 1)).cards[0].id], "trash");
    await m.emptyTrash();
    expect(await m.listTrashed()).toEqual([]);
  });
});
