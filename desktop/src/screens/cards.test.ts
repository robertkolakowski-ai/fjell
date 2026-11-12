import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { Card } from "../bridge/types";
import { CardsSession, loadDeck } from "./cardsFlow";
import { ariaKeys, CARD_KEYS, cardKey, dragIntent, initialCards, reduceCards } from "./cardsState";

const card = (id: number, protectFlags = 0): Card => ({
  id, kind: "blurry", asset: id, group: null, keeper: null, reason: "Blurry", confidence: 0.8, bytes: 10, protectFlags,
});
const loaded = (n: number) => reduceCards(initialCards, { type: "loaded", cards: Array.from({ length: n }, (_, i) => card(i + 1)) });

describe("kortmodus: tilstand", () => {
  it("beskyttede kort kommer aldri i bunken; tom bunke gir deckEmpty", () => {
    const s = reduceCards(initialCards, { type: "loaded", cards: [card(1, 1), card(2)] });
    expect(s.queue.map((c) => c.id)).toEqual([2]);
    expect(reduceCards(initialCards, { type: "loaded", cards: [] }).phase).toBe("deckEmpty");
  });
  it("behold og papirkurv tar øverste kort og teller; siste kort gir deckEmpty", () => {
    let s = loaded(2);
    s = reduceCards(s, { type: "act", action: "trash" });
    s = reduceCards(s, { type: "act", action: "keep" });
    expect(s).toMatchObject({ phase: "deckEmpty", trashed: 1, kept: 1, queue: [] });
  });
  it("hopp over flytter bakerst uten å telle; ett kort igjen kan ikke hoppes over", () => {
    let s = reduceCards(loaded(3), { type: "act", action: "skip" });
    expect(s.queue.map((c) => c.id)).toEqual([2, 3, 1]);
    expect(s.trashed + s.kept + s.history.length).toBe(0);
    s = reduceCards(loaded(1), { type: "act", action: "skip" });
    expect(s.queue).toHaveLength(1);
    expect(s.live).toBeNull();
  });
  it("angre gjentatte ganger, også fra tom bunke", () => {
    let s = loaded(2);
    s = reduceCards(reduceCards(s, { type: "act", action: "trash" }), { type: "act", action: "keep" });
    s = reduceCards(s, { type: "undo" });
    expect(s).toMatchObject({ phase: "card", kept: 0, trashed: 1 });
    s = reduceCards(s, { type: "undo" });
    expect(s.queue.map((c) => c.id)).toEqual([1, 2]);
    expect(reduceCards(s, { type: "undo" })).toBe(s);
  });
  it("rollback gir kortet tilbake og fjerner det fra historikken", () => {
    const s0 = loaded(2);
    const s1 = reduceCards(s0, { type: "act", action: "trash" });
    const s2 = reduceCards(s1, { type: "rollback", card: s0.queue[0], error: { code: "core_error", detail: "" } });
    expect(s2).toMatchObject({ trashed: 0, history: [], error: { code: "core_error" } });
    expect(s2.queue.map((c) => c.id)).toEqual([1, 2]);
  });
  it("live-seq øker for hver annonsering", () => {
    let s = loaded(3);
    s = reduceCards(s, { type: "act", action: "skip" });
    const a = s.live!.seq;
    s = reduceCards(s, { type: "act", action: "skip" });
    expect(s.live!.seq).toBe(a + 1);
  });
  it("zoom lukkes ved beslutning", () => {
    let s = reduceCards(loaded(2), { type: "toggleZoom" });
    expect(s.zoom).toBe(true);
    s = reduceCards(s, { type: "act", action: "keep" });
    expect(s.zoom).toBe(false);
  });
});

describe("kortmodus: taster (6.4)", () => {
  it("tabellen", () => {
    expect(cardKey({ key: "ArrowLeft" })).toBe("keep");
    expect(cardKey({ key: "ArrowUp" })).toBe("trash");
    expect(cardKey({ key: "Delete" })).toBe("trash");
    expect(cardKey({ key: "ArrowRight" })).toBe("skip");
    expect(cardKey({ key: " " })).toBe("zoom");
    expect(cardKey({ key: "z", ctrlKey: true })).toBe("undo");
    expect(cardKey({ key: "F1" })).toBe("help");
  });
  it("aldri i tekstfelt, aldri Alt/Win, ingen enkelttegn, Backspace ubrukt, Mellomrom på knapp aktiverer knappen", () => {
    expect(cardKey({ key: "Delete", inTextField: true })).toBeNull();
    expect(cardKey({ key: "ArrowLeft", altKey: true })).toBeNull();
    expect(cardKey({ key: "ArrowLeft", metaKey: true })).toBeNull();
    expect(cardKey({ key: "k" })).toBeNull();
    expect(cardKey({ key: "Backspace" })).toBeNull();
    expect(cardKey({ key: " ", onButton: true })).toBeNull();
    expect(cardKey({ key: "z", ctrlKey: true, shiftKey: true })).toBeNull();
    expect(cardKey({ key: "w", ctrlKey: true })).toBeNull();
  });
  it("hver tast i tabellen gir sin handling og aria-keyshortcuts", () => {
    for (const k of CARD_KEYS) {
      for (const key of k.keys) {
        const [mod, name] = key.includes("+") ? key.split("+") : [null, key];
        expect(cardKey({ key: name, ctrlKey: mod === "Control" })).toBe(k.id);
      }
      expect(ariaKeys(k.id).split(" ").length).toBe(k.keys.length);
    }
    expect(ariaKeys("zoom")).toBe("Space");
  });
});

describe("kortmodus: mus-dra", () => {
  it("terskel 35 % og retning", () => {
    expect(dragIntent(-30, 0, 100, 100)).toBeNull();
    expect(dragIntent(-36, 0, 100, 100)).toBe("keep");
    expect(dragIntent(36, 5, 100, 100)).toBe("skip");
    expect(dragIntent(0, -36, 100, 100)).toBe("trash");
    expect(dragIntent(0, 80, 100, 100)).toBeNull(); // ned finnes ikke
    expect(dragIntent(-50, 0, 400, 400)).toBeNull();
    expect(dragIntent(-50, 0, 400, 400, 1)).toBe("keep"); // rask sveip
  });
});

describe("kortmodus mot MockBridge", () => {
  it("knapp-flyt: papirkurv, behold, angre gir samme tilstand i kjernen", async () => {
    const b = new MockBridge();
    await b.startScan(false);
    const s = new CardsSession(b, "blurry");
    await s.load();
    expect(s.state.queue).toHaveLength(6);
    await s.act("trash");
    await s.act("keep");
    expect((await b.getState()).trashCount).toBe(1);
    await s.undo(); // angrer behold
    expect((await b.getState()).trashCount).toBe(1);
    await s.undo(); // angrer papirkurv
    expect((await b.getState()).trashCount).toBe(0);
    expect(s.state.queue.map((c) => c.id)).toEqual([...s.state.queue.map((c) => c.id)].sort((a, c) => a - c));
    expect(s.state.queue).toHaveLength(6);
    // kjernen har fått kortene tilbake: ny last gir samme antall
    expect(await loadDeck(b, "blurry")).toHaveLength(6);
  });
  it("tastatur og knapp gir samme batcher", async () => {
    const a = new MockBridge();
    const k = new MockBridge();
    const sa = new CardsSession(a, "screenshot");
    const sk = new CardsSession(k, "screenshot");
    await sa.load(); await sk.load();
    await sa.act("trash"); await sa.act("keep");
    for (const key of ["ArrowUp", "ArrowLeft"]) {
      const id = cardKey({ key })!;
      await sk.act(id as "trash" | "keep");
    }
    expect(await a.getState()).toEqual(await k.getState());
    expect(sa.state.queue).toEqual(sk.state.queue);
  });
  it("alle fire kategorier når kind er null; hele bunken til deckEmpty med oppsummering", async () => {
    const b = new MockBridge();
    const s = new CardsSession(b, null);
    await s.load();
    expect(s.state.queue).toHaveLength(24);
    for (let i = 0; i < 24; i++) await s.act(i % 2 ? "keep" : "trash");
    expect(s.state).toMatchObject({ phase: "deckEmpty", trashed: 12, kept: 12 });
    await s.undo();
    expect(s.state.phase).toBe("card");
  });
  it("kjernefeil ruller tilbake", async () => {
    const b = new MockBridge();
    const s = new CardsSession(b, "blurry");
    await s.load();
    const first = s.state.queue[0];
    b.decide = async () => { throw { code: "core_error", detail: "" }; };
    await s.act("trash");
    expect(s.state.queue[0]).toEqual(first);
    expect(s.state.trashed).toBe(0);
  });
  it("angre venter på ventende beslutninger (seriell kø), også med forsinkelse", async () => {
    const b = new MockBridge({ delayMs: 5 });
    const s = new CardsSession(b, "blurry");
    await s.load();
    void s.act("trash");
    await s.undo();
    expect((await b.getState()).trashCount).toBe(0);
    expect(s.state.queue).toHaveLength(6);
  });
});
