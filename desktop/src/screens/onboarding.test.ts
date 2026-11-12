import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { AppState } from "../bridge/types";
import { quickScan, trashFound } from "./onboardingFlow";
import { initialOnb, needsMassAck, reduceOnb, type OnbEvent, type OnbState } from "./onboardingState";

const run = (evs: OnbEvent[], from: OnbState = initialOnb) => evs.reduce(reduceOnb, from);
const toResult = (count: number, bytes: number): OnbEvent[] => [
  { type: "begin" }, { type: "startScan" }, { type: "exactReady", count, bytes, ids: Array.from({ length: count }, (_, i) => i + 1) },
];
const noSleep = () => Promise.resolve();

describe("onboarding: tilstand", () => {
  it("MassAck ved >= 50 bilder eller >= 1 GB", () => {
    expect(needsMassAck(49, 1024 ** 3 - 1)).toBe(false);
    expect(needsMassAck(50, 0)).toBe(true);
    expect(needsMassAck(1, 1024 ** 3)).toBe(true);
  });
  it("lite funn: ett trykk, ingen dialog", () => {
    const s = run([...toResult(6, 1000), { type: "requestTrash" }]);
    expect(s).toMatchObject({ kind: "result", busy: true, confirming: false });
    expect(run([{ type: "trashed" }], s).kind).toBe("done");
  });
  it("mange funn: krever avkrysning før flytting", () => {
    let s = run([...toResult(312, 2 * 1024 ** 3), { type: "requestTrash" }]);
    expect(s).toMatchObject({ confirming: true, busy: false });
    s = run([{ type: "confirmTrash" }], s);
    expect(s).toMatchObject({ confirming: true, busy: false }); // ikke avkrysset
    s = run([{ type: "setAck", acked: true }, { type: "confirmTrash" }], s);
    expect(s).toMatchObject({ confirming: false, busy: true });
  });
  it("avbryt MassAck nullstiller avkrysning", () => {
    const s = run([...toResult(60, 1), { type: "requestTrash" }, { type: "setAck", acked: true }, { type: "cancelConfirm" }]);
    expect(s).toMatchObject({ confirming: false, acked: false, busy: false });
  });
  it("0 funn gir nothingFound uten seier", () => {
    expect(run(toResult(0, 0)).kind).toBe("nothingFound");
  });
  it("angre fra seier går tilbake til resultat", () => {
    const s = run([...toResult(3, 10), { type: "requestTrash" }, { type: "trashed" }, { type: "undo" }, { type: "undone" }]);
    expect(s).toMatchObject({ kind: "result", count: 3, busy: false });
  });
  it("feil ved flytting beholder resultat med feilflagg", () => {
    expect(run([...toResult(3, 10), { type: "requestTrash" }, { type: "trashFailed" }])).toMatchObject({ kind: "result", failed: true, busy: false });
  });
  it("langsom skann gir foreløpig resultat som kan bli ferdig", () => {
    let s = run([{ type: "begin" }, { type: "startScan" }, { type: "progress", found: 4 }, { type: "slow" }]);
    expect(s).toEqual({ kind: "partial", found: 4 });
    s = run([{ type: "exactReady", count: 5, bytes: 9, ids: [1] }], s);
    expect(s.kind).toBe("result");
  });
  it("nektet tilgang gir noAccess og vei tilbake", () => {
    const s = run([{ type: "begin" }, { type: "startScan" }, { type: "scanDenied" }]);
    expect(s.kind).toBe("noAccess");
    expect(reduceOnb(s, { type: "chooseAgain" }).kind).toBe("chooseFolders");
  });
  it("ugyldige hendelser ignoreres", () => {
    expect(run([{ type: "trashed" }, { type: "confirmTrash" }, { type: "undo" }])).toEqual(initialOnb);
  });
});

describe("onboarding: flyt mot MockBridge", () => {
  it("skann → første funn → papirkurv → angre", async () => {
    const b = new MockBridge();
    const seen: number[] = [];
    const r = await quickScan(b, { sleep: noSleep, onFound: (n) => seen.push(n) });
    expect(r.kind).toBe("ready");
    if (r.kind !== "ready") return;
    expect(r.found.count).toBe(6);
    expect(r.found.ids).toHaveLength(6);
    expect(r.found.bytes).toBeGreaterThan(0);
    await trashFound(b, r.found.ids);
    const st = await b.getState();
    expect(st.trashCount).toBe(6);
    expect(st.perKind.find((k) => k.kind === "exact_dup")?.count).toBe(0);
    await b.undo();
    expect((await b.getState()).trashCount).toBe(0);
  });
  it("avvist startScan med no_access gir denied", async () => {
    class Denied extends MockBridge {
      override async startScan(): Promise<void> { throw { code: "no_access", detail: "" }; }
      override async getState(): Promise<AppState> { return { ...(await super.getState()), scanPhase: "listing" }; }
    }
    expect(await quickScan(new Denied(), { sleep: noSleep })).toEqual({ kind: "denied" });
  });
  it("annen feil gir failed", async () => {
    class Broken extends MockBridge {
      override async startScan(): Promise<void> { throw { code: "core_error", detail: "" }; }
      override async getState(): Promise<AppState> { return { ...(await super.getState()), scanPhase: "listing" }; }
    }
    expect(await quickScan(new Broken(), { sleep: noSleep })).toEqual({ kind: "failed" });
  });
  it("sendt foreløpig-signal etter 45 s, så ferdig", async () => {
    let clock = 0;
    let polls = 0;
    class Slow extends MockBridge {
      override async startScan(): Promise<void> { /* skann pågår */ }
      override async getState(): Promise<AppState> {
        const s = await super.getState();
        polls++;
        return { ...s, scanPhase: polls < 5 ? "exact" : "exact_ready" };
      }
    }
    let slow = 0;
    const r = await quickScan(new Slow(), {
      sleep: async () => { clock += 20_000; }, now: () => clock, onSlow: () => slow++,
    });
    expect(slow).toBe(1);
    expect(r.kind).toBe("ready");
  });
  it("0 eksakte duplikater gir ready med count 0", async () => {
    class Clean extends MockBridge {
      override async getState(): Promise<AppState> {
        const s = await super.getState();
        return { ...s, perKind: s.perKind.map((k) => (k.kind === "exact_dup" ? { ...k, count: 0, bytes: 0 } : k)) };
      }
    }
    const r = await quickScan(new Clean(), { sleep: noSleep });
    expect(r).toEqual({ kind: "ready", found: { count: 0, bytes: 0, ids: [] } });
  });
});
