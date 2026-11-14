import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { FacesSession } from "./facesFlow";
import { PeopleSession } from "./peopleFlow";
import { defaultKeep, mergeRoles, validateName } from "./peopleState";

const labels = () => ({ unnamed: "Uten navn" });

async function loaded(faceIndex: boolean) {
  const b = new MockBridge();
  if (faceIndex) await b.setFaceIndex(true);
  const s = new PeopleSession(b, labels);
  await s.load();
  return { b, s };
}

describe("personer: indeks av (standard)", () => {
  it("viser ingen personer og ingenting skjer før samtykke", async () => {
    const { b, s } = await loaded(false);
    expect(s.state).toMatchObject({ phase: "ready", faceIndex: false, people: [], consent: false });
    s.openConsent();
    expect(s.state.consent).toBe(true);
    expect(await b.getFaceIndex()).toBe(false);
    s.closeConsent();
    expect(await b.getFaceIndex()).toBe(false);
  });
  it("samtykke slår på indeksen og viser personene", async () => {
    const { b, s } = await loaded(false);
    s.openConsent();
    expect(await s.enable()).toBe(true);
    expect(await b.getFaceIndex()).toBe(true);
    expect(s.state.people).toHaveLength(4);
    expect(s.state.consent).toBe(false);
  });
});

describe("personer: navn", () => {
  it("lagrer navn og kan angre", async () => {
    const { b, s } = await loaded(true);
    s.startRename(2);
    await s.saveName(2, "  Eva  ");
    expect(s.state.people.find((p) => p.id === 2)?.name).toBe("Eva");
    expect(s.state.editing).toBeNull();
    expect(await s.undo()).toBe(true);
    expect((await b.listPeople()).find((p) => p.id === 2)?.name).toBeNull();
  });
  it("tomt navn fjerner navnet, angre gir det tilbake", async () => {
    const { b, s } = await loaded(true);
    await s.saveName(1, "");
    expect((await b.listPeople()).find((p) => p.id === 1)?.name).toBeNull();
    await s.undo();
    expect((await b.listPeople()).find((p) => p.id === 1)?.name).toBe("Kari");
  });
  it("for langt navn avvises før kall", async () => {
    const { s } = await loaded(true);
    s.startRename(2);
    await s.saveName(2, "x".repeat(101));
    expect(s.state.nameError).toBe("name_too_long");
    expect(s.state.editing).toBe(2);
    expect(validateName("x".repeat(100))).toBeNull();
  });
  it("uendret navn gir ingen angre-handling", async () => {
    const { s } = await loaded(true);
    s.startRename(1);
    await s.saveName(1, "Kari");
    expect(s.state.undo).toBeNull();
    expect(s.state.editing).toBeNull();
  });
});

describe("personer: sammenslåing", () => {
  it("velger standard: person med navn blir stående", async () => {
    const { s } = await loaded(true);
    s.openMerge(2);
    s.pickMergeOther(1);
    expect(s.state.merge).toMatchObject({ from: 2, other: 1, keep: "other" });
    expect(mergeRoles(s.state.merge!)).toEqual({ target: 1, absorbed: 2 });
  });
  it("slår sammen, viser færre personer og angrer med unmergePerson", async () => {
    const { b, s } = await loaded(true);
    s.openMerge(2);
    s.pickMergeOther(3);
    s.setMergeKeep("other");
    expect(await s.confirmMerge()).toBe(true);
    expect(s.state.people.map((p) => p.id)).toEqual([1, 3, 4]);
    expect(s.state.undo).toMatchObject({ kind: "merge", absorbed: 2, target: 3 });
    expect(s.state.merge).toBeNull();
    expect(await s.undo()).toBe(true);
    expect((await b.listPeople()).map((p) => p.id).sort()).toEqual([1, 2, 3, 4]);
    expect(s.state.undo).toBeNull();
  });
  it("ingen sammenslåing uten valgt person; avbryt rører ingenting", async () => {
    const { b, s } = await loaded(true);
    s.openMerge(1);
    expect(await s.confirmMerge()).toBe(false);
    s.cancelMerge();
    expect(s.state.merge).toBeNull();
    expect(await b.listPeople()).toHaveLength(4);
  });
  it("defaultKeep: ved likt velges flest ansikter", () => {
    const base = { id: 0, name: null, assetCount: 1, cover: null, protected: false };
    expect(defaultKeep({ ...base, faceCount: 3 }, { ...base, faceCount: 9 })).toBe("other");
    expect(defaultKeep({ ...base, faceCount: 9 }, { ...base, faceCount: 3 })).toBe("from");
  });
});

describe("personer: beskyttelse", () => {
  it("beskytter og kan angre", async () => {
    const { b, s } = await loaded(true);
    await s.toggleProtect(1, true);
    expect(s.state.people.find((p) => p.id === 1)?.protected).toBe(true);
    expect((await b.listPeople()).find((p) => p.id === 1)?.protected).toBe(true);
    await s.undo();
    expect((await b.listPeople()).find((p) => p.id === 1)?.protected).toBe(false);
  });
  it("feil gir kode og ingen angre-handling", async () => {
    const { b, s } = await loaded(true);
    b.protectPerson = () => Promise.reject({ code: "unknown_person", detail: "" });
    await s.toggleProtect(1, true);
    expect(s.state).toMatchObject({ error: "unknown_person", undo: null, busy: false });
  });
});

describe("ansiktsindeks i innstillinger", () => {
  async function faces(on = false) {
    const b = new MockBridge();
    if (on) await b.setFaceIndex(true);
    const s = new FacesSession(b);
    await s.load();
    return { b, s };
  }
  it("er av som standard, og På krever samtykke", async () => {
    const { b, s } = await faces();
    expect(s.state.enabled).toBe(false);
    s.requestOn();
    expect(await b.getFaceIndex()).toBe(false);
    s.cancelConsent();
    expect(s.state).toMatchObject({ consent: false, enabled: false });
    s.requestOn();
    await s.confirmOn();
    expect(await b.getFaceIndex()).toBe(true);
    expect(s.state.enabled).toBe(true);
  });
  it("Av slår bare av analysen og sletter ingenting", async () => {
    const { b, s } = await faces(true);
    await s.turnOff();
    expect(s.state.enabled).toBe(false);
    await b.setFaceIndex(true);
    expect(await b.listPeople()).toHaveLength(4);
  });
  it("wipe kjører aldri uten bekreftelsesdialog", async () => {
    const { b, s } = await faces(true);
    let calls = 0;
    const orig = b.wipeFaceIndex.bind(b);
    b.wipeFaceIndex = () => { calls++; return orig(); };
    expect(await s.confirmWipe()).toBe(false);
    expect(calls).toBe(0);
    s.requestWipe();
    s.cancelWipe();
    expect(await s.confirmWipe()).toBe(false);
    expect(calls).toBe(0);
    s.requestWipe();
    expect(await s.confirmWipe()).toBe(true);
    expect(calls).toBe(1);
    expect(s.state.wiped).toBe(33);
    expect(await b.listPeople()).toEqual([]);
  });
  it("wipe-feil gir feilkode og lukker dialogen", async () => {
    const { b, s } = await faces(true);
    b.wipeFaceIndex = () => Promise.reject({ code: "core_error", detail: "" });
    s.requestWipe();
    await s.confirmWipe();
    expect(s.state).toMatchObject({ confirmWipe: false, error: "core_error", wiped: null });
  });
});
