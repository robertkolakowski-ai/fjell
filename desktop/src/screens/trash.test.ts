import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { TrashSession } from "./trashFlow";
import { ackFor, daysLeft, hasExpired, infoOf, initialTrash, reduceTrash, sectionsOf } from "./trashState";

async function withTrash(opts = {}, n = 3) {
  const b = new MockBridge(opts);
  await b.startScan(false);
  const ids = (await b.listCards("blurry", 50)).cards.slice(0, n).map((c) => c.id);
  await b.decide(ids, "trash");
  const s = new TrashSession(b);
  await s.load();
  return { b, s };
}

describe("papirkurv: tilstand", () => {
  it("tom og full", async () => {
    const b = new MockBridge();
    const s = new TrashSession(b);
    await s.load();
    expect(s.state.phase).toBe("empty");
    const w = await withTrash();
    expect(w.s.state).toMatchObject({ phase: "list", info: { count: 3 } });
  });
  it("feil ved lasting", () => {
    expect(reduceTrash(initialTrash, { type: "failed", error: { code: "core_error", detail: "" } }).phase).toBe("error");
  });
  it("utløpt vises, aldri stille", () => {
    const info = infoOf({ backend: "mock", perKind: [], reclaimableBytes: 0, trashCount: 2, trashBytes: 1, trashExpired: 2, freedBytes: 0, scanPhase: "done", scanDone: 0, scanTotal: 0, trashDays: 30 });
    expect(hasExpired(info)).toBe(true);
    expect(hasExpired(null)).toBe(false);
  });
  it("ark: Papirkurv er standard, Avbryt lukker, ingenting skjer før Fortsett", async () => {
    const { b, s } = await withTrash();
    s.send({ type: "openPurge" });
    expect(s.state).toMatchObject({ phase: "confirmPurge", mode: "recycle" });
    s.send({ type: "cancel" });
    expect(s.state.phase).toBe("list");
    expect((await b.getState()).trashCount).toBe(3);
  });
  it("permanent: ekstra trinn, krever avkrysning, antall følger med", async () => {
    const { s } = await withTrash();
    s.send({ type: "openPurge" });
    s.send({ type: "setMode", mode: "permanent" });
    s.send({ type: "continue" });
    expect(s.state.phase).toBe("confirmPermanent");
    s.send({ type: "confirmPermanent" });
    expect(s.state.phase).toBe("confirmPermanent"); // uten avkrysning skjer ingenting
    s.send({ type: "setAck", value: true });
    expect(ackFor(s.state)).toEqual({ checked: true, count: 3 });
    expect(ackFor({ ...s.state, mode: "recycle" })).toBeNull();
  });
});

describe("papirkurv mot MockBridge", () => {
  it("Papirkurv-modus: ingenting frigjort, ingen historikk", async () => {
    const { b, s } = await withTrash();
    s.send({ type: "openPurge" });
    await s.confirm();
    expect(s.state).toMatchObject({ phase: "purged", outcome: { mode: "recycle", count: 3, freedBytes: 0 } });
    expect((await b.getState()).freedBytes).toBe(0);
    expect(s.state.info?.count).toBe(0);
    s.send({ type: "dismiss" });
    expect(s.state.phase).toBe("empty");
  });
  it("permanent: frigjort og historikk, kun etter avkrysning", async () => {
    const { b, s } = await withTrash();
    s.send({ type: "openPurge" });
    s.send({ type: "setMode", mode: "permanent" });
    await s.confirm(); // Fortsett
    expect(s.state.phase).toBe("confirmPermanent");
    await s.confirm(); // uten avkrysning: ingenting
    expect((await b.getState()).trashCount).toBe(3);
    s.send({ type: "setAck", value: true });
    await s.confirm();
    expect(s.state).toMatchObject({ phase: "purged", outcome: { mode: "permanent", count: 3 } });
    const st = await b.getState();
    expect(st.freedBytes).toBe(s.state.outcome!.freedBytes);
    expect(st.freedSince).toBeTypeOf("number");
  });
  it("bridge avviser permanent uten avkrysning eller feil antall", async () => {
    const { b } = await withTrash();
    await expect(b.emptyTrash({ checked: false, count: 3 })).rejects.toMatchObject({ code: "ack_required" });
    await expect(b.emptyTrash({ checked: true, count: 2 })).rejects.toMatchObject({ code: "ack_required" });
    expect((await b.getState()).trashCount).toBe(3);
  });
  it("volum uten Papirkurv: ingenting slettet, bare permanent tilbys, aldri automatisk", async () => {
    const { b, s } = await withTrash({ noBin: true });
    s.send({ type: "openPurge" });
    await s.confirm();
    expect(s.state).toMatchObject({ phase: "confirmPurge", noBin: true, mode: "permanent" });
    expect((await b.getState()).trashCount).toBe(3);
    s.send({ type: "setMode", mode: "recycle" });
    expect(s.state.mode).toBe("permanent");
    s.send({ type: "cancel" });
    s.send({ type: "openPurge" });
    expect(s.state).toMatchObject({ phase: "confirmPurge", noBin: true, mode: "permanent" });
  });
  it("feil under tømming: ingenting slettet, tilbake til listen", async () => {
    const { b, s } = await withTrash();
    b.emptyTrash = async () => { throw { code: "core_error", detail: "" }; };
    s.send({ type: "openPurge" });
    await s.confirm();
    expect(s.state).toMatchObject({ phase: "list", error: { code: "core_error" } });
    expect((await b.getState()).trashCount).toBe(3);
  });
});

describe("papirkurv: per-bilde-liste og gjenoppretting", () => {
  const DAY = 86_400_000;
  it("listen har ett element per bilde, med filnavn, størrelse og datoer", async () => {
    const { s } = await withTrash({}, 3);
    expect(s.state.items).toHaveLength(3);
    for (const i of s.state.items) {
      expect(i.name).toMatch(/^IMG_\d{4}\.jpg$/);
      expect(i.bytes).toBeGreaterThan(0);
      expect(i.expiresAt - i.trashedAt).toBe(30 * 86_400);
      expect(i.expired).toBe(false);
    }
  });
  it("valg: toggle, velg alle, fjern; ukjent bilde ignoreres", async () => {
    const { s } = await withTrash({}, 3);
    const [a, b2] = s.state.items;
    s.send({ type: "toggle", asset: a.asset });
    s.send({ type: "toggle", asset: b2.asset });
    s.send({ type: "toggle", asset: b2.asset });
    expect([...s.state.selected]).toEqual([a.asset]);
    s.send({ type: "toggle", asset: 99999 });
    expect(s.state.selected.size).toBe(1);
    s.send({ type: "selectAll" });
    expect(s.state.selected.size).toBe(3);
    s.send({ type: "clearSelection" });
    expect(s.state.selected.size).toBe(0);
  });
  it("Gjenopprett: bare valgte, umiddelbart, uten bekreftelsesark", async () => {
    const { b, s } = await withTrash({}, 3);
    const keep = s.state.items[2].asset;
    s.send({ type: "toggle", asset: s.state.items[0].asset });
    s.send({ type: "toggle", asset: s.state.items[1].asset });
    await s.restore(false);
    expect(s.state).toMatchObject({ phase: "list", restored: 2, restoring: false, restoreFailed: false });
    expect(s.state.items.map((i) => i.asset)).toEqual([keep]);
    expect(s.state.selected.size).toBe(0);
    expect((await b.getState()).trashCount).toBe(1);
  });
  it("Gjenopprett uten valg gjør ingenting", async () => {
    const { b, s } = await withTrash({}, 2);
    await s.restore(false);
    expect(s.state.restored).toBeNull();
    expect((await b.getState()).trashCount).toBe(2);
  });
  it("Gjenopprett alt tømmer listen og gir tomtilstand", async () => {
    const { b, s } = await withTrash({}, 3);
    await s.restore(true);
    expect(s.state).toMatchObject({ phase: "empty", restored: 3 });
    expect(s.state.items).toEqual([]);
    expect((await b.getState()).trashCount).toBe(0);
  });
  it("feil: ingenting endres, melding vises, listen står urørt", async () => {
    const { b, s } = await withTrash({}, 2);
    b.restoreAll = () => Promise.reject({ code: "core_error", detail: "" });
    await s.restore(true);
    expect(s.state).toMatchObject({ phase: "list", restoreFailed: true, restoring: false });
    expect(s.state.items).toHaveLength(2);
    // Prøv igjen virker (restoring er ikke låst).
    s.send({ type: "toggle", asset: s.state.items[0].asset });
    expect(s.state.selected.size).toBe(1);
  });
  it("dobbelttrykk under gjenoppretting gir ett kall", async () => {
    const { b, s } = await withTrash({ delayMs: 1 }, 2);
    let calls = 0;
    const orig = b.restoreAll.bind(b);
    b.restoreAll = () => { calls++; return orig(); };
    await Promise.all([s.restore(true), s.restore(true)]);
    expect(calls).toBe(1);
  });
  it("grupper: utløpt, utløper snart (≤ 7 dager), øvrige; tomme utelates", async () => {
    let t = 1_000_000_000_000;
    const { b, s } = await withTrash({ now: () => t }, 3);
    const nowS = Math.floor(t / 1000);
    expect(sectionsOf(s.state.items, nowS).map((x) => x.id)).toEqual(["other"]);
    t += 25 * DAY; // 5 dager igjen
    await s.load();
    expect(sectionsOf(s.state.items, Math.floor(t / 1000)).map((x) => x.id)).toEqual(["soon"]);
    expect(daysLeft(s.state.items[0], Math.floor(t / 1000))).toBe(5);
    t += 6 * DAY; // utløpt
    await s.load();
    expect(sectionsOf(s.state.items, Math.floor(t / 1000)).map((x) => x.id)).toEqual(["expired"]);
    expect(s.state.info?.expired).toBe(3);
    expect(daysLeft(s.state.items[0], Math.floor(t / 1000))).toBe(0);
    // Utløpt er ikke slettet: bildene kan fortsatt gjenopprettes.
    await s.restore(true);
    expect((await b.getState()).trashCount).toBe(0);
  });
});
