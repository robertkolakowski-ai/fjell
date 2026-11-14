import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { KIND_CODES } from "../bridge/types";
import type { Dashboard } from "../bridge/types-v1";
import { DashboardSession } from "./dashboardFlow";
import { cleanable, freedTotals, initialDashboard, isEmpty, kindTiles, monthDate, nextTarget, progressFraction, reduceDashboard } from "./dashboardState";

const empty: Dashboard = {
  activeCount: 0, activeBytes: 0, trashedCount: 0, trashedBytes: 0, freedBytes: 0, perKind: [], byMonth: [], topFolders: [],
  protectedCount: 0, seriesCount: 0, peopleCount: 0, rulesActive: 0,
  progress: { decided: 0, total: 0, permille: 0 }, nextStep: { kind: "nothing", suggestionKind: null, count: 0, bytes: 0 }, freedByMonth: [],
};
const err = { code: "core_error", detail: "" };

describe("oversikt: tilstand", () => {
  it("tom, klar og feil", () => {
    expect(isEmpty(empty)).toBe(true);
    expect(reduceDashboard(initialDashboard, { type: "loaded", data: empty }).kind).toBe("empty");
    const ready = reduceDashboard(initialDashboard, { type: "loaded", data: { ...empty, trashedCount: 1 } });
    expect(ready.kind).toBe("ready");
    expect(reduceDashboard(initialDashboard, { type: "failed", error: err }).kind).toBe("error");
    expect(reduceDashboard(ready, { type: "failed", error: err })).toBe(ready);
    expect(reduceDashboard({ kind: "error", error: err }, { type: "retry" }).kind).toBe("loading");
  });
  it("fremdrift klemmes til 0..1", () => {
    expect(progressFraction({ ...empty, progress: { decided: 1, total: 4, permille: 250 } })).toBe(0.25);
    expect(progressFraction({ ...empty, progress: { decided: 0, total: 0, permille: 5000 } })).toBe(1);
    expect(progressFraction({ ...empty, progress: { decided: 0, total: 0, permille: Number.NaN } })).toBe(0);
  });
  it("neste grep: kode til mål, ukjent type gir hele Gjennomgang, nothing gir ingen knapp", () => {
    const step = (kind: Dashboard["nextStep"]["kind"], suggestionKind: string | null) => ({ ...empty, nextStep: { kind, suggestionKind, count: 1, bytes: 1 } });
    expect(nextTarget(step("review", "blurry"))).toEqual({ to: "review", kind: "blurry" });
    expect(nextTarget(step("review", "ukjent"))).toEqual({ to: "review", kind: null });
    expect(nextTarget(step("empty_trash", null))).toEqual({ to: "trash" });
    expect(nextTarget(step("nothing", null))).toBeNull();
  });
  it("tømt per måned og månedsdato", () => {
    expect(freedTotals([{ yearMonth: 202610, count: 2, bytes: 5 }, { yearMonth: 202611, count: 1, bytes: 3 }])).toEqual({ count: 3, bytes: 8 });
    expect(monthDate(202611).toISOString()).toBe("2026-11-01T00:00:00.000Z");
  });
});

describe("oversikt mot MockBridge", () => {
  it("før skann er oversikten tom", async () => {
    const s = new DashboardSession(new MockBridge());
    await s.load();
    expect(s.state.kind).toBe("empty");
  });
  it("etter skann: alle seks typer, summen er kan ryddes, og tallene stemmer med Hjem og Gjennomgang", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const s = new DashboardSession(b);
    await s.load();
    expect(s.state.kind).toBe("ready");
    const d = await b.getDashboard();
    expect(kindTiles(d).map((k) => k.kind)).toEqual([...KIND_CODES]);
    const home = await b.getState();
    expect(cleanable(d).bytes).toBe(home.reclaimableBytes);
    expect(cleanable(d).count).toBe(home.perKind.reduce((n, k) => n + k.count, 0));
    expect(nextTarget(d)).toMatchObject({ to: "review" });
  });
  it("beskyttede regelkort teller ikke som kan ryddes, og beskyttelse fjerner ikke tallene for beskyttede", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const before = cleanable(await b.getDashboard());
    const rule = (await b.listCards("rule", 50)).cards[0];
    await b.protect({ scope: "asset", target: String(rule.asset) }, true);
    const after = await b.getDashboard();
    expect(after.protectedCount).toBe(1);
    expect(cleanable(after).count).toBe(before.count - 1);
  });
  it("papirkurv, tømt og angre følger kjernens tall; «Tømt» er 0 før noe er tømt", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const [c] = (await b.listCards("blurry", 10)).cards;
    await b.decide([c.id], "trash");
    let d = await b.getDashboard();
    expect(d).toMatchObject({ trashedCount: 1, freedBytes: 0, freedByMonth: [] });
    expect(d.nextStep.kind).toBe("review");
    await b.purge();
    d = await b.getDashboard();
    expect(d.freedBytes).toBe(c.bytes);
    expect(freedTotals(d.freedByMonth)).toEqual({ count: 1, bytes: c.bytes });
  });
  it("neste grep er empty_trash når alt er avgjort, og ingen handling utføres av oversikten", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const ids = (await Promise.all(KIND_CODES.map((k) => b.listCards(k, 50)))).flatMap((r) => r.cards.map((c) => c.id));
    await b.decide(ids, "trash");
    const s = new DashboardSession(b);
    await s.load();
    const d = await b.getDashboard();
    expect(nextTarget(d)).toEqual({ to: "trash" });
    expect((await b.getState()).trashCount).toBe(d.trashedCount);
  });
  it("feil fra bridgen gir feiltilstand og retry laster på nytt", async () => {
    const b = new MockBridge();
    const orig = b.getDashboard.bind(b);
    let fail = true;
    b.getDashboard = async () => { if (fail) throw err; return orig(); };
    const s = new DashboardSession(b);
    await s.load();
    expect(s.state.kind).toBe("error");
    fail = false;
    await s.retry();
    expect(s.state.kind).toBe("empty");
  });
});
