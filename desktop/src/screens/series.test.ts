import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { BridgeError, Card } from "../bridge/types";
import { fmt, setLanguage, t } from "../lib/i18n";
import { initialCards, reduceCards } from "./cardsState";
import { explainBody, explainFacts } from "./explain";
import { initialGroup, reduceGroup } from "./groupState";
import { isProtectedAck, PROTECTED_ACK } from "./protectAck";
import { initialReview, reduceReview } from "./reviewState";
import {
  allCardIds, initialSeries, keeperForKey, loadSeries, markedBytes, moveIds, reduceSeries, seriesItems, ZOOM_STEPS, zoomKey,
  type SeriesEvent, type SeriesState,
} from "./seriesState";

const run = (s: SeriesState, ...es: SeriesEvent[]) => es.reduce(reduceSeries, s);
const ack: BridgeError = { code: "ack_required", detail: PROTECTED_ACK };

async function ready(b: MockBridge, id = 41): Promise<SeriesState> {
  const r = await loadSeries(b, id);
  return run(initialSeries, { type: "loaded", compare: r.compare, cards: r.cards });
}

describe("serie: forhåndsmerking", () => {
  it("alt unntatt behold-kandidaten er forhåndsmerket, og behold-kandidaten kan ikke merkes", async () => {
    const s = await ready(new MockBridge());
    const items = seriesItems(s);
    expect(items).toHaveLength(4);
    expect(items[0]).toMatchObject({ n: 1, rank: 1, best: true, marked: false, markable: false });
    expect(items.slice(1).every((i) => i.marked && i.markable)).toBe(true);
    expect(moveIds(s)).toHaveLength(3);
    expect(allCardIds(s)).toHaveLength(3);
    expect(s.cardOf.has(items[0].asset)).toBe(false);
    expect(run(s, { type: "toggleMark", asset: items[0].asset }).marked).toEqual(s.marked);
  });
  it("beskyttede bilder er aldri forhåndsmerket og kan ikke merkes", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const first = (await loadSeries(b, 41)).compare.members[1].asset;
    await b.protect({ scope: "asset", target: String(first) }, true);
    const s = await ready(b);
    const it = seriesItems(s).find((i) => i.asset === first)!;
    expect(it).toMatchObject({ protected: true, marked: false, markable: false });
    expect(moveIds(s)).toHaveLength(2);
    expect(run(s, { type: "toggleMark", asset: first }).marked.has(first)).toBe(false);
  });
  it("avmerking tar bildet ut av papirkurv-valget, og sum følger", async () => {
    const s0 = await ready(new MockBridge());
    const asset = seriesItems(s0)[1].asset;
    const s1 = run(s0, { type: "toggleMark", asset });
    expect(moveIds(s1)).toHaveLength(2);
    expect(markedBytes(s1)).toBeLessThan(markedBytes(s0));
    expect(moveIds(run(s1, { type: "toggleMark", asset }))).toHaveLength(3);
  });
  it("lukkede øyne er et eget tekstmerke på medlemmet", async () => {
    const items = seriesItems(await ready(new MockBridge(), 42));
    expect(items.filter((i) => i.closedEyes)).toHaveLength(1);
    expect(items[0].reason).toBe("KeeperEyesOpen");
  });
});

describe("serie: tilstand", () => {
  it("handling bare fra ready; bekreftelse for beskyttede er et spørsmål, ikke en feil", async () => {
    const s = await ready(new MockBridge());
    expect(reduceSeries(initialSeries, { type: "start" })).toBe(initialSeries);
    const w = run(s, { type: "start" });
    expect(w.phase).toBe("working");
    const asked = run(w, { type: "askProtected" });
    expect(asked).toMatchObject({ phase: "ready", askProtected: true, error: null });
    expect(run(asked, { type: "dismissAsk" }).askProtected).toBe(false);
    expect(run(w, { type: "actionFailed", error: ack })).toMatchObject({ phase: "ready", error: ack });
    expect(run(w, { type: "committed", done: "trash", moved: 3 })).toMatchObject({ phase: "committed", done: "trash", moved: 3 });
  });
  it("zoom er felles, har faste trinn og klemmes i begge ender", () => {
    let s = initialSeries;
    for (let i = 0; i < 9; i++) s = reduceSeries(s, { type: "zoomIn" });
    expect(s.zoom).toBe(ZOOM_STEPS[ZOOM_STEPS.length - 1]);
    s = reduceSeries(s, { type: "zoomOut" });
    expect(s.zoom).toBe(ZOOM_STEPS[ZOOM_STEPS.length - 2]);
    expect(reduceSeries(s, { type: "zoomReset" }).zoom).toBe(1);
    expect(reduceSeries(initialSeries, { type: "zoomOut" }).zoom).toBe(1);
  });
  it("taster: 1–9 velger beste (ikke nåværende beste), +, − og 0 zoomer, Ctrl gir ingenting", async () => {
    const items = seriesItems(await ready(new MockBridge()));
    expect(keeperForKey(items, "1")).toBeNull();
    expect(keeperForKey(items, "3")).toBe(items[2].asset);
    expect(keeperForKey(items, "9")).toBeNull();
    expect(keeperForKey(items, "0")).toBeNull();
    expect(zoomKey({ key: "+" })).toBe("zoomIn");
    expect(zoomKey({ key: "-" })).toBe("zoomOut");
    expect(zoomKey({ key: "0" })).toBe("zoomReset");
    expect(zoomKey({ key: "+", ctrlKey: true })).toBeNull();
    expect(zoomKey({ key: "a" })).toBeNull();
  });
});

describe("serie mot MockBridge", () => {
  it("«Gjør dette til beste» bytter behold-kandidat og forhåndsmerker på nytt", async () => {
    const b = new MockBridge();
    const s0 = await ready(b);
    const pick = seriesItems(s0)[2].asset;
    const oldBest = seriesItems(s0)[0].asset;
    await b.setKeeper(41, pick);
    const r = await loadSeries(b, 41);
    const s1 = run(s0, { type: "start" }, { type: "refreshed", compare: r.compare, cards: r.cards });
    const items = seriesItems(s1);
    expect(items[0]).toMatchObject({ asset: pick, best: true, marked: false });
    expect(items.find((i) => i.asset === oldBest)).toMatchObject({ best: false, marked: true });
    expect(moveIds(s1)).toHaveLength(3);
  });
  it("refresh beholder brukerens valg: fjernet hake kommer ikke tilbake, nylig ubeskyttet blir ikke forhåndsmerket", async () => {
    const b = new MockBridge();
    const s0 = await ready(b);
    const [a, , c] = seriesItems(s0).slice(1).map((i) => i.asset);
    const s1 = run(s0, { type: "toggleMark", asset: a });
    const before = moveIds(s1);
    // Endret beskyttelse på c og tilbake igjen: a skal fortsatt være umerket.
    await b.protect({ scope: "asset", target: String(c) }, true);
    const r1 = await loadSeries(b, 41);
    const s2 = run(s1, { type: "start" }, { type: "refreshed", compare: r1.compare, cards: r1.cards });
    expect(seriesItems(s2).find((i) => i.asset === a)?.marked).toBe(false);
    expect(seriesItems(s2).find((i) => i.asset === c)).toMatchObject({ protected: true, marked: false });
    await b.protect({ scope: "asset", target: String(c) }, false);
    const r2 = await loadSeries(b, 41);
    const s3 = run(s2, { type: "start" }, { type: "refreshed", compare: r2.compare, cards: r2.cards });
    expect(seriesItems(s3).find((i) => i.asset === a)?.marked).toBe(false);
    expect(seriesItems(s3).find((i) => i.asset === c)?.marked).toBe(false);
    expect(moveIds(s3)).toHaveLength(before.length - 1);
  });
  it("flytt resten er én batch som angres i ett grep", async () => {
    const b = new MockBridge();
    const s = await ready(b);
    await b.decide(moveIds(s), "trash");
    await expect(b.compareSeries(41)).rejects.toMatchObject({ code: "unknown_series" });
    await b.undo();
    expect((await loadSeries(b, 41)).cards).toHaveLength(3);
  });
  it("behold alle er også ett angrebart valg, og bildene blir ikke foreslått igjen", async () => {
    const b = new MockBridge();
    const s = await ready(b);
    await b.decide(allCardIds(s), "keep");
    await expect(b.compareSeries(41)).rejects.toMatchObject({ code: "unknown_series" });
    await b.undo();
    expect((await loadSeries(b, 41)).cards).toHaveLength(3);
  });
  it("ukjent serie gir unknown_series", async () => {
    await expect(loadSeries(new MockBridge(), 999)).rejects.toMatchObject({ code: "unknown_series" });
  });
});

describe("beskyttede bilder: bekreftelse (felles for Gjennomgang, Kortmodus, Gruppe)", () => {
  const card: Card = { id: 1, kind: "series", asset: 1, group: 41, keeper: 9, reason: "SeriesMember", detail: { kind: "none" }, confidence: 1, bytes: 5, protectFlags: 0 };
  it("kjenner koden", () => {
    expect(isProtectedAck(ack)).toBe(true);
    expect(isProtectedAck({ code: "ack_required", detail: "permanent" })).toBe(false);
    expect(isProtectedAck(null)).toBe(false);
  });
  it("Gjennomgang: avslag blir spørsmål, bekreftelse starter nytt kall, avbrudd rører ingenting", () => {
    let s = reduceReview(initialReview, { type: "loaded", cards: [card], next: null });
    s = reduceReview(s, { type: "toggle", id: 1 });
    s = reduceReview(s, { type: "requestCommit" });
    s = reduceReview(s, { type: "commitFailed", error: ack });
    expect(s).toMatchObject({ phase: "list", askProtected: true, error: null });
    expect(s.selected.has(1)).toBe(true);
    expect(reduceReview(s, { type: "cancelProtected" })).toMatchObject({ askProtected: false, phase: "list" });
    const go = reduceReview(s, { type: "confirmProtected" });
    expect(go).toMatchObject({ phase: "committing", protectedOk: true, askProtected: false });
    expect(reduceReview(go, { type: "committed", batch: 3 }).protectedOk).toBe(false);
  });
  it("Kortmodus: kortet ligger tilbake øverst og spørsmålet er åpent", () => {
    let s = reduceCards(initialCards, { type: "loaded", cards: [card] });
    s = reduceCards(s, { type: "act", action: "trash" });
    s = reduceCards(s, { type: "rollback", card, error: ack });
    expect(s).toMatchObject({ phase: "card", askProtected: true, error: null, trashed: 0 });
    expect(s.queue.map((c) => c.id)).toEqual([1]);
    expect(reduceCards(s, { type: "dismissAsk" }).askProtected).toBe(false);
  });
  it("Gruppe: samme spørsmål", () => {
    let s = reduceGroup(initialGroup, { type: "loaded", group: { id: 1, keeper: 9, cards: [card], bytes: 5 } });
    s = reduceGroup(s, { type: "start" });
    expect(reduceGroup(s, { type: "askProtected" })).toMatchObject({ phase: "ready", askProtected: true });
  });
});

describe("forklaring av begrunnelsesmerke", () => {
  setLanguage("nb");
  it("serieposisjon, antall og regel kommer fra detail", () => {
    const names = new Map([[1, "Gamle skjermbilder"]]);
    expect(explainBody({ reason: "SeriesMember", detail: { kind: "position", index: 2, of: 4 } }, t)).toContain("2");
    expect(explainFacts({ reason: "SeriesMember", detail: { kind: "position", index: 2, of: 4 } }, t)).toEqual([
      { code: "position", label: "Plass i serien", value: "2 av 4" },
    ]);
    expect(explainFacts({ reason: "ClosedEyes", detail: { kind: "count", value: 3 } }, t)[0]).toMatchObject({ code: "count", value: "3" });
    expect(explainBody({ reason: "ClosedEyes", detail: { kind: "count", value: 3 } }, t)).toContain("3 ansikter");
    expect(explainBody({ reason: "Rule", detail: { kind: "rule", id: 1 }, ruleNames: names }, t)).toContain("Gamle skjermbilder");
    expect(explainFacts({ reason: "Rule", detail: { kind: "rule", id: 1 }, ruleNames: names }, t)[0].value).toBe("Gamle skjermbilder");
    expect(explainFacts({ reason: "Rule", detail: { kind: "rule", id: 7 }, ruleNames: names }, t)[0].value).toBe("uten navn");
  });
  it("prosent: detail.percent eller kortets confidence (uskarphet); person og ingenting", () => {
    expect(explainFacts({ reason: "Blurry", detail: { kind: "percent", value: 0.82 } }, t)[0]).toMatchObject({ code: "blur", value: fmt.percent(0.82) });
    expect(explainFacts({ reason: "Blurry", detail: { kind: "none" }, confidence: 0.9 }, t)[0].value).toBe(fmt.percent(0.9));
    expect(explainBody({ reason: "Blurry", detail: { kind: "none" }, confidence: 0.9 }, t)).toContain(fmt.percent(0.9));
    expect(explainFacts({ reason: "ProtectedPerson", detail: { kind: "person", id: 3 } }, t)[0].code).toBe("person");
    expect(explainFacts({ reason: "ExactCopy", detail: { kind: "none" } }, t)).toEqual([]);
  });
  it("alle begrunnelser har en forklaring som ikke er tom og ikke gir råd", () => {
    const all = ["ExactCopy", "VisualDuplicate", "Blurry", "Screenshot", "HigherResolution", "LessCompressed", "Sharper", "Older",
      "ShotPlatformFlag", "ShotScreenSize", "ShotNoCameraExif", "ShotPng", "SeriesMember", "KeeperBestOverall", "KeeperEyesOpen",
      "ClosedEyes", "Rule", "ProtectedPerson"] as const;
    for (const reason of all) {
      const text = explainBody({ reason, detail: { kind: "none" }, confidence: 0.5 }, t);
      expect(text.length, reason).toBeGreaterThan(10);
      expect(text, reason).not.toMatch(/bør|må |slett/i);
    }
  });
});
