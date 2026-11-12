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

import { addFolder, controlScan } from "./homeFlow";
import { neverScanned } from "./homeState";
import { markOnboarded, shouldShowOnboarding } from "./onboardingGate";
import { HOME, parseHash } from "../router";

describe("hjem: skannkontroller", () => {
  it("pause → fortsett → ferdig, og state følger bridge", async () => {
    const b = new MockBridge({ scanPolls: 5 });
    await b.startScan(true);
    expect(reduceHome(initialHome, { type: "loaded", summary: await b.getState() }).kind).toBe("scanning");
    expect(await controlScan(b, "pause")).toBeNull();
    const paused = reduceHome(initialHome, { type: "loaded", summary: await b.getState() });
    expect(paused.kind).toBe("paused");
    // Pauset skann gjør ingen fremdrift selv om vi poller.
    for (let i = 0; i < 10; i++) await b.getState();
    expect((await b.getState()).scanPaused).toBe(true);
    expect(await controlScan(b, "resume")).toBeNull();
    expect((await b.getState()).scanPaused).toBe(false);
    let st = await b.getState();
    for (let i = 0; i < 10 && st.scanPhase !== "done"; i++) st = await b.getState();
    expect(st.scanPhase).toBe("done");
  });
  it("avbryt beholder funnet og gir melding «cancelled»", async () => {
    const b = new MockBridge({ scanPolls: 50 });
    await b.startScan(true);
    expect(await controlScan(b, "cancel")).toBe("cancelled");
    const st = await b.getState();
    expect(st.scanPaused).toBe(false);
    expect(st.perKind.reduce((n, k) => n + k.count, 0)).toBeGreaterThan(0);
    expect(reduceHome(initialHome, { type: "loaded", summary: st }).kind).toBe("ready");
  });
  it("feil i skannkontroll gir «error», ingen krasj", async () => {
    const b = new MockBridge({ library: null });
    expect(await controlScan(b, "start")).toBe("error");
    b.pauseScan = () => Promise.reject({ code: "core_error", detail: "" });
    expect(await controlScan(b, "pause")).toBe("error");
  });
  it("aldri skannet: tilbyr Start skanning, ikke «alt ryddet»", async () => {
    const b = new MockBridge();
    expect(neverScanned(await b.getState())).toBe(true);
    await controlScan(b, "start");
    expect(neverScanned(await b.getState())).toBe(false);
  });
});

describe("hjem: Legg til mappe", () => {
  it("velger mappe og starter skann", async () => {
    const b = new MockBridge({ library: null });
    expect(await b.getLibrary()).toBeNull();
    const r = await addFolder(b, "  D:\\Ferie  ");
    expect(r).toEqual({ ok: true, path: "D:\\Ferie" });
    expect(await b.getLibrary()).toBe("D:\\Ferie");
    expect((await b.getState()).scanPhase).toBe("done");
  });
  it("tom sti avvises uten kall, forrige mappe beholdes", async () => {
    const b = new MockBridge({ library: "C:\\Bilder" });
    expect(await addFolder(b, "   ")).toMatchObject({ ok: false, error: { code: "not_a_folder" } });
    expect(await b.getLibrary()).toBe("C:\\Bilder");
  });
  it("feil fra bridge gir ok:false og ingen endring", async () => {
    const b = new MockBridge({ library: "C:\\Bilder" });
    b.setLibrary = () => Promise.reject({ code: "not_a_folder", detail: "x" });
    expect(await addFolder(b, "Z:\\finnes-ikke")).toMatchObject({ ok: false });
    expect(await b.getLibrary()).toBe("C:\\Bilder");
  });
  it("onboarding: scan=false setter bare mappe", async () => {
    const b = new MockBridge({ library: null });
    await addFolder(b, "D:\\X", false);
    expect((await b.getState()).scanPhase).toBe("idle");
  });
});

describe("første gang", () => {
  it("onboarding vises bare første gang og bare fra Hjem", async () => {
    const b = new MockBridge({ firstRun: true, library: null });
    expect(await shouldShowOnboarding(b, HOME)).toBe(true);
    expect(await shouldShowOnboarding(b, parseHash("#/trash"))).toBe(false);
    expect(await shouldShowOnboarding(b, parseHash("#/onboarding"))).toBe(false);
  });
  it("etter gjennomført onboarding (skann startet) vises den aldri igjen", async () => {
    const b = new MockBridge({ firstRun: true, library: null });
    await markOnboarded(b);
    expect(await shouldShowOnboarding(b, HOME)).toBe(false);
  });
  it("å velge mappe avslutter første gang", async () => {
    const b = new MockBridge({ firstRun: true, library: null });
    await b.setLibrary("D:\\Bilder");
    expect(await b.getFirstRun()).toBe(false);
  });
  it("vanlig oppstart og feil gir aldri onboarding", async () => {
    expect(await shouldShowOnboarding(new MockBridge(), HOME)).toBe(false);
    const b = new MockBridge({ firstRun: true, library: null });
    b.getFirstRun = () => Promise.reject({ code: "core_error", detail: "" });
    expect(await shouldShowOnboarding(b, HOME)).toBe(false);
  });
});
