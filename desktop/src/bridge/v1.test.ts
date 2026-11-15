// V1 i broen: TauriBridge kaller riktige kommandoer; MockBridge har samme semantikk som kjernen (V1-PLAN del 7).
import { describe, expect, it, vi } from "vitest";
import { MockBridge } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";
import { KIND_CODES } from "./types";
import type { Card } from "./types";

const mock = (o: ConstructorParameters<typeof MockBridge>[0] = {}) => new MockBridge({ now: () => Date.UTC(2026, 10, 14), ...o });
const all = async (b: MockBridge, kind: Parameters<MockBridge["listCards"]>[0]): Promise<Card[]> => (await b.listCards(kind, 200)).cards;

describe("TauriBridge: V1-kommandoer", () => {
  it("hver metode kaller kommandoen med samme navn og argumenter som skallet forventer", async () => {
    const call = vi.fn().mockResolvedValue(undefined);
    const b = new TauriBridge(call);
    const scope = { scope: "folder", target: "C:\\Bilder" } as const;
    await b.protect(scope, true);
    await b.listProtections();
    await b.listRules();
    await b.saveRule({ id: null, name: "R", conditions: [{ kind: "is_blurry" }] });
    await b.previewRule([{ kind: "older_than_days", value: 90 }]);
    await b.toggleRule(3, false);
    await b.deleteRule(3);
    await b.restoreRule(3);
    await b.listPeople();
    await b.namePerson(1, null);
    await b.mergePeople(1, 2);
    await b.unmergePerson(2);
    await b.protectPerson(1, true);
    await b.getFaceIndex();
    await b.setFaceIndex(true);
    await b.wipeFaceIndex();
    await b.getDashboard();
    await b.compareSeries(41);
    await b.getSuggestionSettings();
    await b.getThresholds();
    await b.decide([1], "trash", ["protected_by_scope"]);
    await b.decide([1], "keep");
    expect(call.mock.calls).toEqual([
      ["protect", { scope, on: true }],
      ["list_protections"],
      ["list_rules"],
      ["save_rule", { draft: { id: null, name: "R", conditions: [{ kind: "is_blurry" }] } }],
      ["preview_rule", { conditions: [{ kind: "older_than_days", value: 90 }] }],
      ["toggle_rule", { id: 3, enabled: false }],
      ["delete_rule", { id: 3 }],
      ["restore_rule", { id: 3 }],
      ["list_people"],
      ["name_person", { id: 1, name: null }],
      ["merge_people", { target: 1, absorbed: 2 }],
      ["unmerge_person", { absorbed: 2 }],
      ["protect_person", { id: 1, protect: true }],
      ["get_face_index"],
      ["set_face_index", { enabled: true }],
      ["wipe_face_index"],
      ["get_dashboard"],
      ["compare_series", { series: 41 }],
      ["get_suggestion_settings"],
      ["get_thresholds"],
      ["decide_cards", { ids: [1], decision: "trash", confirmed: ["protected_by_scope"] }],
      ["decide_cards", { ids: [1], decision: "keep" }],
    ]);
  });
  it("setSuggestionSettings sender hele innstillingen", async () => {
    const call = vi.fn().mockResolvedValue({});
    const s = { trashDays: 14, sensitivity: Object.fromEntries(KIND_CODES.map((k) => [k, "normal"])), faceIndex: false } as never;
    await new TauriBridge(call).setSuggestionSettings(s);
    expect(call.mock.calls).toEqual([["set_suggestion_settings", { settings: s }]]);
  });
});

describe("MockBridge: forslagstyper", () => {
  it("seks typer med kort; serier og regler har detail og begrunnelse", async () => {
    const b = mock();
    await b.startScan(false);
    const st = await b.getState();
    expect(st.perKind.map((k) => k.kind)).toEqual([...KIND_CODES]);
    expect(st.perKind.every((k) => k.count === 6)).toBe(true);
    const series = await all(b, "series");
    expect(series.map((c) => c.group)).toEqual([41, 41, 41, 42, 42, 42]);
    expect(series[0]).toMatchObject({ reason: "SeriesMember", detail: { kind: "position", index: 2, of: 4 } });
    expect(series[3]).toMatchObject({ reason: "ClosedEyes", detail: { kind: "count", value: 1 } });
    const rules = await all(b, "rule");
    expect(rules.every((c) => c.reason === "Rule" && c.detail.kind === "rule")).toBe(true);
  });
  it("compareSeries: behold-kandidat først, flere medlemmer enn kort, ukjent serie gir unknown_series", async () => {
    const b = mock();
    const c = await b.compareSeries(41);
    expect(c.members).toHaveLength(4);
    expect(c.members[0]).toMatchObject({ rank: 1, isKeeper: true, asset: c.keeper, reason: "KeeperBestOverall" });
    expect(c.members.slice(1).every((m) => !m.isKeeper)).toBe(true);
    expect((await b.compareSeries(42)).members[0].reason).toBe("KeeperEyesOpen");
    await expect(b.compareSeries(999)).rejects.toMatchObject({ code: "unknown_series" });
  });
  it("setKeeper bytter behold-kandidat i en serie, og compareSeries følger", async () => {
    const b = mock();
    const before = await b.compareSeries(41);
    const pick = before.members[2].asset;
    await b.setKeeper(41, pick);
    const after = await b.compareSeries(41);
    expect(after.keeper).toBe(pick);
    expect(after.members.map((m) => m.asset).sort()).toEqual(before.members.map((m) => m.asset).sort());
  });
});

describe("MockBridge: beskyttelse", () => {
  it("protect/angre er et par; beskyttede bilder får flagg og er aldri forhåndsmerket, og papirkurv krever egen bekreftelse", async () => {
    const b = mock();
    const [first] = await all(b, "blurry");
    await b.protect({ scope: "asset", target: String(first.asset) }, true);
    expect((await b.listProtections()).map((p) => p.scope)).toEqual([{ scope: "asset", target: String(first.asset) }]);
    const flagged = (await all(b, "blurry")).find((c) => c.id === first.id)!;
    expect(flagged.protectFlags).not.toBe(0);
    await expect(b.decide([first.id], "trash")).rejects.toMatchObject({ code: "ack_required", detail: "protected_by_scope" });
    expect((await b.getState()).trashCount).toBe(0);
    await b.decide([first.id], "trash", ["protected_by_scope"]);
    expect((await b.getState()).trashCount).toBe(1);
    await b.undo();
    await b.protect({ scope: "asset", target: String(first.asset) }, false);
    expect(await b.listProtections()).toEqual([]);
    expect((await all(b, "blurry")).find((c) => c.id === first.id)!.protectFlags).toBe(0);
  });
  it("behold krever ingen bekreftelse", async () => {
    const b = mock();
    const [c] = await all(b, "blurry");
    await b.protect({ scope: "asset", target: String(c.asset) }, true);
    await expect(b.decide([c.id], "keep")).resolves.toBeGreaterThan(0);
  });
  it("feilkoder: tom mappe, ukjent bilde/album/person, ugyldig omfang", async () => {
    const b = mock();
    await expect(b.protect({ scope: "folder", target: "  \\" }, true)).rejects.toMatchObject({ code: "empty_folder" });
    await expect(b.protect({ scope: "asset", target: "999999" }, true)).rejects.toMatchObject({ code: "unknown_target" });
    await expect(b.protect({ scope: "album", target: "77" }, true)).rejects.toMatchObject({ code: "unknown_target" });
    await expect(b.protect({ scope: "person", target: "1" }, true)).rejects.toMatchObject({ code: "unknown_target" });
    await expect(b.protect({ scope: "nope" as never, target: "1" }, true)).rejects.toMatchObject({ code: "bad_request" });
  });
  it("mappe og album beskytter bildene i dem; å slå på to ganger gir én rad", async () => {
    const b = mock();
    await b.protect({ scope: "album", target: "1" }, true);
    await b.protect({ scope: "album", target: "1" }, true);
    expect(await b.listProtections()).toHaveLength(1);
    const shots = await all(b, "screenshot");
    expect(shots.filter((c) => c.protectFlags !== 0).length).toBeGreaterThan(0);
    await b.protect({ scope: "folder", target: "C:\\Bilder\\Ferie\\" }, true);
    expect((await b.listProtections()).map((p) => p.scope.target)).toContain("C:\\Bilder\\Ferie");
    const vis = await all(b, "visual_dup");
    expect(vis.some((c) => c.protectFlags !== 0)).toBe(true);
  });
});

describe("MockBridge: regler", () => {
  it("regler foreslår bare: lagre gir kort, ingenting flyttes til papirkurv", async () => {
    const b = mock();
    await b.startScan(false);
    const before = (await b.getState()).perKind.find((k) => k.kind === "rule")!.count;
    const id = await b.saveRule({ id: null, name: "  Små filer ", conditions: [{ kind: "smaller_than_bytes", value: 500_000 }] });
    expect(id).toBeGreaterThan(2);
    const rules = await b.listRules();
    expect(rules.find((r) => r.id === id)).toMatchObject({ name: "Små filer", enabled: true });
    expect(rules.find((r) => r.id === id)!.hits).toBeGreaterThan(0);
    expect((await b.getState()).perKind.find((k) => k.kind === "rule")!.count).toBeGreaterThan(before);
    expect((await b.getState()).trashCount).toBe(0);
  });
  it("forhåndsvisning teller treff uten å lagre, og er lik det som lagres", async () => {
    const b = mock();
    const cond = [{ kind: "is_blurry" }, { kind: "older_than_days", value: 30 }] as const;
    const p = await b.previewRule([...cond]);
    expect(p.hits).toBeGreaterThan(0);
    expect(p.sample.length).toBeLessThanOrEqual(3);
    expect((await b.listRules()).length).toBe(2);
    const id = await b.saveRule({ id: null, name: "X", conditions: [...cond] });
    expect((await b.listRules()).find((r) => r.id === id)!.hits).toBe(p.hits);
  });
  it("beskyttede bilder gir aldri regeltreff; fjernes beskyttelsen, kommer treffet tilbake", async () => {
    const b = mock();
    const cond = [{ kind: "is_blurry" }] as const;
    const p = await b.previewRule([...cond]);
    await b.protect({ scope: "asset", target: String(p.sample[0]) }, true);
    expect((await b.previewRule([...cond])).hits).toBe(p.hits - 1);
    const id = await b.saveRule({ id: null, name: "B", conditions: [...cond] });
    expect((await all(b, "rule")).some((c) => c.asset === p.sample[0])).toBe(false);
    await b.protect({ scope: "asset", target: String(p.sample[0]) }, false);
    expect((await all(b, "rule")).some((c) => c.asset === p.sample[0])).toBe(true);
    expect((await b.listRules()).find((r) => r.id === id)!.hits).toBe(p.hits);
  });
  it("av/på skjuler og viser regelens kort; slett kan angres med restoreRule", async () => {
    const b = mock();
    const count = async () => (await all(b, "rule")).length;
    expect(await count()).toBe(6);
    await b.toggleRule(1, false);
    expect(await count()).toBe(3);
    expect((await b.listRules()).find((r) => r.id === 1)).toMatchObject({ enabled: false, hits: 0 });
    await b.toggleRule(1, true);
    expect(await count()).toBe(6);
    await b.deleteRule(2);
    expect((await b.listRules()).map((r) => r.id)).toEqual([1]);
    expect(await count()).toBe(3);
    await b.restoreRule(2);
    expect((await b.listRules()).map((r) => r.id)).toEqual([1, 2]);
    expect(await count()).toBe(6);
  });
  it("feilkoder som i V1-PLAN", async () => {
    const b = mock();
    await expect(b.saveRule({ id: null, name: "A", conditions: [] })).rejects.toMatchObject({ code: "empty_rule" });
    // Som kjernen: forhåndsvisning validerer ikke; en tom regel treffer ingenting (aldri alt).
    await expect(b.previewRule([])).resolves.toMatchObject({ hits: 0 });
    await expect(b.saveRule({ id: null, name: "  ", conditions: [{ kind: "is_blurry" }] })).rejects.toMatchObject({ code: "empty_name" });
    await expect(b.saveRule({ id: null, name: "x".repeat(81), conditions: [{ kind: "is_blurry" }] })).rejects.toMatchObject({ code: "name_too_long" });
    await expect(b.saveRule({ id: null, name: "A", conditions: [{ kind: "in_folder", value: " " }] })).rejects.toMatchObject({ code: "invalid_condition" });
    // Kjernen tar imot tall fra 0 (skjermen krever mer, se `conditionsError`); negative og brøk er ikke gyldige tall for kjernen.
    await expect(b.saveRule({ id: null, name: "A", conditions: [{ kind: "older_than_days", value: -1 }] })).rejects.toMatchObject({ code: "invalid_condition" });
    await expect(b.saveRule({ id: null, name: "A0", conditions: [{ kind: "older_than_days", value: 0 }] })).resolves.toBeGreaterThan(0);
    await expect(b.saveRule({ id: 99, name: "A", conditions: [{ kind: "is_blurry" }] })).rejects.toMatchObject({ code: "rule_not_found" });
    await expect(b.toggleRule(99, true)).rejects.toMatchObject({ code: "rule_not_found" });
    await expect(b.deleteRule(99)).rejects.toMatchObject({ code: "rule_not_found" });
    await expect(b.restoreRule(99)).rejects.toMatchObject({ code: "rule_not_found" });
    await b.deleteRule(1);
    await expect(b.deleteRule(1)).rejects.toMatchObject({ code: "rule_not_found" });
  });
  it("endring av en regel bytter treffene", async () => {
    const b = mock();
    await b.saveRule({ id: 1, name: "Endret", conditions: [{ kind: "mime", value: "image/png" }] });
    expect((await b.listRules()).find((r) => r.id === 1)!.name).toBe("Endret");
    expect((await all(b, "rule")).every((c) => c.asset >= 201 || c.detail.kind === "rule" && c.detail.id === 2)).toBe(true);
  });
});

describe("MockBridge: personer og ansiktsindeks", () => {
  it("ansiktsindeksen er av som standard og gir ingen personer", async () => {
    const b = mock();
    expect(await b.getFaceIndex()).toBe(false);
    expect(await b.listPeople()).toEqual([]);
    expect((await b.getSuggestionSettings()).faceIndex).toBe(false);
    await expect(b.namePerson(1, "A")).rejects.toMatchObject({ code: "unknown_person" });
  });
  it("på: personer vises; navn, sammenslåing og angre", async () => {
    const b = mock();
    await b.setFaceIndex(true);
    const ps = await b.listPeople();
    expect(ps.map((p) => p.id)).toEqual([1, 2, 3, 4]);
    expect(ps[0]).toMatchObject({ name: "Kari", protected: false });
    await b.namePerson(2, "  Per ");
    await b.namePerson(1, null);
    expect((await b.listPeople()).slice(0, 2).map((p) => p.name)).toEqual([null, "Per"]);
    await expect(b.namePerson(1, "x".repeat(101))).rejects.toMatchObject({ code: "name_too_long" });
    const f = (await b.listPeople()).find((p) => p.id === 3)!;
    expect(await b.mergePeople(2, 3)).toBe(2);
    const merged = (await b.listPeople()).find((p) => p.id === 2)!;
    expect(merged.faceCount).toBeGreaterThan(6);
    expect(merged.faceCount).toBe(6 + f.faceCount);
    expect((await b.listPeople()).map((p) => p.id)).toEqual([1, 2, 4]);
    expect(await b.mergePeople(2, 2)).toBe(2);
    await b.unmergePerson(3);
    expect((await b.listPeople()).map((p) => p.id)).toEqual([1, 2, 3, 4]);
    expect((await b.listPeople()).find((p) => p.id === 2)!.faceCount).toBe(6);
    await expect(b.unmergePerson(3)).rejects.toMatchObject({ code: "not_merged" });
    await expect(b.unmergePerson(99)).rejects.toMatchObject({ code: "unknown_person" });
    await expect(b.mergePeople(1, 99)).rejects.toMatchObject({ code: "unknown_person" });
  });
  it("personbeskyttelse virker på personens bilder; wipe nuller personer og gjør beskyttelsen til bildebeskyttelse", async () => {
    const b = mock();
    await b.setFaceIndex(true);
    await b.protectPerson(1, true);
    expect((await b.listPeople())[0].protected).toBe(true);
    expect((await b.listProtections()).map((p) => p.scope.scope)).toEqual(["person"]);
    const before = await all(b, "exact_dup");
    expect(before.filter((c) => [1, 2, 3, 4].includes(c.asset)).every((c) => c.protectFlags !== 0)).toBe(true);
    expect(await b.wipeFaceIndex()).toBe(33);
    expect(await b.listPeople()).toEqual([]);
    const prot = await b.listProtections();
    expect(prot.every((p) => p.scope.scope === "asset")).toBe(true);
    expect(prot.map((p) => p.scope.target).sort()).toEqual(["1", "2", "3", "4", "7", "8"]);
    // Ingen bilder mister vernet.
    const dup = await all(b, "exact_dup");
    expect(dup.filter((c) => [1, 2, 3, 4].includes(c.asset)).every((c) => c.protectFlags !== 0)).toBe(true);
    expect((await b.getDashboard()).peopleCount).toBe(0);
  });
  it("indeksen bygges på nytt ved skanning etter wipe når den er på", async () => {
    const b = mock();
    await b.setFaceIndex(true);
    await b.wipeFaceIndex();
    expect(await b.listPeople()).toEqual([]);
    await b.startScan(false);
    expect((await b.listPeople()).length).toBe(4);
  });
  it("personbeskyttelse kan fjernes (angre)", async () => {
    const b = mock();
    await b.setFaceIndex(true);
    await b.protectPerson(4, true);
    await b.protectPerson(4, false);
    expect(await b.listProtections()).toEqual([]);
    await expect(b.protectPerson(99, true)).rejects.toMatchObject({ code: "unknown_person" });
  });
});

describe("MockBridge: dashboard, innstillinger og terskler", () => {
  it("dashboard speiler tilstanden og påstår ikke frigjort plass før noe er tømt", async () => {
    const b = mock();
    await b.startScan(false);
    let d = await b.getDashboard();
    expect(d.freedBytes).toBe(0);
    expect(d.freedByMonth).toEqual([]);
    expect(d.perKind.map((k) => k.kind)).toEqual([...KIND_CODES]);
    expect(d.seriesCount).toBe(2);
    expect(d.rulesActive).toBe(2);
    expect(d.progress).toMatchObject({ decided: 0, total: 36, permille: 0 });
    expect(d.nextStep.kind).toBe("review");
    expect(d.byMonth).toHaveLength(6);
    expect(d.byMonth[5].yearMonth).toBe(202611);
    const [c] = await all(b, "blurry");
    await b.decide([c.id], "trash");
    d = await b.getDashboard();
    expect(d.trashedCount).toBe(1);
    expect(d.progress.decided).toBe(1);
    expect(d.freedBytes).toBe(0);
    await b.purge();
    d = await b.getDashboard();
    expect(d.freedBytes).toBe(c.bytes);
    expect(d.freedByMonth).toEqual([{ yearMonth: 202611, count: 1, bytes: c.bytes }]);
  });
  it("neste steg: tøm papirkurven når alle forslag er avgjort", async () => {
    const b = mock();
    await b.startScan(false);
    const ids = (await Promise.all(KIND_CODES.map((k) => all(b, k)))).flat().map((c) => c.id);
    await b.decide(ids, "trash");
    const d = await b.getDashboard();
    expect(d.nextStep).toMatchObject({ kind: "empty_trash", count: 36 });
    await b.undo();
    expect((await b.getDashboard()).nextStep.kind).toBe("review");
  });
  it("forslagsinnstillinger: standard normal, ansiktsindeks av; lagring og validering", async () => {
    const b = mock();
    const s = await b.getSuggestionSettings();
    expect(s).toMatchObject({ trashDays: 30, faceIndex: false });
    expect(Object.keys(s.sensitivity)).toEqual([...KIND_CODES]);
    expect(Object.values(s.sensitivity).every((v) => v === "normal")).toBe(true);
    const next = await b.setSuggestionSettings({ ...s, trashDays: 14, sensitivity: { ...s.sensitivity, visual_dup: "high", series: "low" }, faceIndex: true });
    expect(next).toMatchObject({ trashDays: 14, faceIndex: true });
    expect(next.sensitivity.visual_dup).toBe("high");
    expect((await b.getSettings()).trashDays).toBe(14);
    expect(await b.getFaceIndex()).toBe(true);
    await expect(b.setSuggestionSettings({ ...s, trashDays: 0 })).rejects.toMatchObject({ code: "bad_request" });
    await expect(b.setSuggestionSettings({ ...s, sensitivity: { ...s.sensitivity, blurry: "extreme" as never } })).rejects.toMatchObject({ code: "bad_request" });
    expect((await b.getSuggestionSettings()).trashDays).toBe(14);
  });
  it("terskler følger sensitiviteten (V1-PLAN del 7)", async () => {
    const b = mock();
    expect(await b.getThresholds()).toEqual({ visualMaxHamming: 6, blurMaxScore: 0.006, seriesGapSecs: 10, closedEyesMinConfidence: 0.8 });
    const s = await b.getSuggestionSettings();
    await b.setSuggestionSettings({ ...s, sensitivity: { ...s.sensitivity, visual_dup: "high", blurry: "low", series: "high" } });
    expect(await b.getThresholds()).toEqual({ visualMaxHamming: 8, blurMaxScore: 0.0042, seriesGapSecs: 20, closedEyesMinConfidence: 0.7 });
  });
});

describe("TauriBridge: feilkoder", () => {
  it("løfter kjent V1-kode fra detail til code; andre feil går uendret videre", async () => {
    const call = vi.fn().mockRejectedValueOnce({ code: "core_error", detail: "empty_rule" })
      .mockRejectedValueOnce({ code: "core_error", detail: "noe_annet" })
      .mockRejectedValueOnce({ code: "plan_stale", detail: "" });
    const b = new TauriBridge(call);
    await expect(b.listRules()).rejects.toEqual({ code: "empty_rule", detail: "" });
    await expect(b.listRules()).rejects.toEqual({ code: "core_error", detail: "noe_annet" });
    await expect(b.listRules()).rejects.toEqual({ code: "plan_stale", detail: "" });
  });
});
