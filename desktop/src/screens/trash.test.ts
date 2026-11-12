import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { TrashSession } from "./trashFlow";
import { ackFor, hasExpired, infoOf, initialTrash, reduceTrash } from "./trashState";

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
