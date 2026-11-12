import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { AppState } from "../bridge/types";
import { initialHome, kindOf, reduceHome, ringFraction } from "./homeState";

const base: AppState = {
  backend: "mock", perKind: [], reclaimableBytes: 0, trashCount: 0, trashBytes: 0, trashExpired: 0,
  freedBytes: 0, scanPhase: "done", scanDone: 0, scanTotal: 0, trashDays: 30,
};
const load = (summary: AppState) => reduceHome(initialHome, { type: "loaded", summary });

describe("hjem: tilstand", () => {
  it("under skann vises fremdrift", () => {
    expect(load({ ...base, scanPhase: "analyzing", scanDone: 4210, scanTotal: 10000 })).toMatchObject({ kind: "scanning", done: 4210, total: 10000 });
  });
  it("ferdig uten forslag er allClean, aldri skannet er ikke det", () => {
    expect(load(base).kind).toBe("allClean");
    expect(load({ ...base, scanPhase: "idle" }).kind).toBe("ready");
  });
  it("forslag gir ready", () => {
    expect(load({ ...base, perKind: [{ kind: "blurry", count: 2, bytes: 5 }] }).kind).toBe("ready");
  });
  it("feil ved første henting, men ikke under polling", () => {
    const err = { code: "core_error", detail: "" };
    expect(reduceHome(initialHome, { type: "failed", error: err }).kind).toBe("error");
    const ready = load({ ...base, perKind: [{ kind: "blurry", count: 2, bytes: 5 }] });
    expect(reduceHome(ready, { type: "failed", error: err })).toBe(ready);
    expect(reduceHome({ kind: "error", error: err }, { type: "retry" }).kind).toBe("loading");
  });
  it("ring: andel og tomt", () => {
    expect(ringFraction(base)).toBe(0);
    expect(ringFraction({ ...base, reclaimableBytes: 3, trashBytes: 1 })).toBe(0.75);
    expect(kindOf(base, "blurry")).toEqual({ kind: "blurry", count: 0, bytes: 0 });
  });
});

describe("hjem mot MockBridge", () => {
  it("før skann: ready med nuller; etter skann: kategorier; etter papirkurv: papirkurvtall", async () => {
    const b = new MockBridge();
    expect(load(await b.getState())).toMatchObject({ kind: "ready" });
    await b.startScan(true);
    const s = await b.getState();
    expect(load(s).kind).toBe("ready");
    expect(s.perKind.every((k) => k.count === 6)).toBe(true);
    const page = await b.listCards("exact_dup", 200);
    await b.decide(page.cards.map((c) => c.id), "trash");
    const after = await b.getState();
    expect(after.trashCount).toBe(6);
    expect(after.reclaimableBytes).toBeLessThan(s.reclaimableBytes);
  });
});
