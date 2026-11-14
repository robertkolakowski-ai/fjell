import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { Condition } from "../bridge/types-v1";
import { nb } from "../lib/i18n/strings/nb";
import { setLanguage, t } from "../lib/i18n";
import { canSave, editorFor, previewIsFresh, runPreview, RulesSession, saveEditor } from "./rulesFlow";
import { bestUnit, conditionOf, conditionsOf, describeCondition, describeRule, errorKeyOf, newRow, rowFromCondition, CONDITION_KINDS } from "./rulesState";
import { ProtectionsSession, describeTarget } from "./protectionsFlow";
import { SensitivitySession, SENSITIVE_KINDS } from "./sensitivityFlow";

const bytes = (n: number): string => `${n} B`;

beforeAll(() => setLanguage("nb"));
afterAll(() => setLanguage("en"));

describe("vilkårsbygger (rulesState)", () => {
  it("dekker alle vilkårstyper og gir rader tilbake som samme vilkår", () => {
    const all: Condition[] = [
      { kind: "is_screenshot" }, { kind: "is_blurry" }, { kind: "is_exact_duplicate" }, { kind: "older_than_days", value: 90 },
      { kind: "larger_than_bytes", value: 20_000_000 }, { kind: "smaller_than_bytes", value: 1_500 }, { kind: "in_album", value: 2 },
      { kind: "in_folder", value: "D:\\Bilder\\Ferie" }, { kind: "mime", value: "image/png" },
    ];
    expect(new Set(all.map((c) => c.kind))).toEqual(new Set(CONDITION_KINDS));
    for (const c of all) expect(conditionOf(rowFromCondition(c))).toEqual(c);
  });
  it("størrelse velger største hele enhet", () => {
    expect(bestUnit(20_000_000)).toEqual({ value: 20, unit: "mb" });
    expect(bestUnit(3_000_000_000)).toEqual({ value: 3, unit: "gb" });
    expect(bestUnit(1_500)).toEqual({ value: 1.5, unit: "kb" });
  });
  it("ugyldig innhold sendes videre uendret slik at bridgen svarer med feilkode", () => {
    const r = newRow("older_than_days");
    expect(conditionOf(r)).toEqual({ kind: "older_than_days", value: 0 });
    r.text = "abc";
    expect(conditionOf(r)).toEqual({ kind: "older_than_days", value: 0 });
    r.text = "2,5";
    expect(conditionOf({ ...r, kind: "larger_than_bytes", unit: "mb" })).toEqual({ kind: "larger_than_bytes", value: 2_500_000 });
    expect(conditionOf({ ...newRow("mime"), text: "  " })).toEqual({ kind: "mime", value: "" });
  });
  it("lesbar setning bruker bare i18n og formaterer størrelse", () => {
    const s = describeRule([{ kind: "is_screenshot" }, { kind: "older_than_days", value: 90 }, { kind: "larger_than_bytes", value: 5 }], t, bytes);
    expect(s).toBe("Foreslår bilder der alle disse stemmer: Er et skjermbilde; Eldre enn 90 dager; Større enn 5 B.");
    expect(describeCondition({ kind: "in_folder", value: "D:\\Ferie" }, t, bytes)).toBe("I mappen D:\\Ferie");
  });
  it("hver feilkode fra bridgen har tekst; ukjent kode gir generell feil", () => {
    for (const code of ["empty_rule", "empty_name", "name_too_long", "invalid_condition", "rule_not_found"]) {
      expect(errorKeyOf({ code, detail: "" })).toBe(`error.${code}`);
      expect(nb[`error.${code}` as keyof typeof nb]).toBeTruthy();
    }
    expect(errorKeyOf({ code: "ukjent", detail: "" })).toBe("rules.error.generic");
    expect(errorKeyOf(null)).toBe("rules.error.generic");
  });
});

describe("regelliste (RulesSession)", () => {
  async function loaded() {
    const b = new MockBridge();
    const s = new RulesSession(b);
    await s.load();
    return { b, s };
  }
  it("viser reglene med treff og på/av", async () => {
    const { s } = await loaded();
    expect(s.state.phase).toBe("ready");
    expect(s.state.rules.map((r) => r.name)).toEqual(["Gamle skjermbilder", "Store filer"]);
  });
  it("av/på oppdaterer listen og gir melding; avslått regel har 0 treff", async () => {
    const { s } = await loaded();
    const id = s.state.rules[0].id;
    expect(await s.toggle(id, false)).toBe(true);
    expect(s.state.rules[0]).toMatchObject({ enabled: false, hits: 0 });
    expect(s.state.note).toEqual({ kind: "toggled", name: "Gamle skjermbilder", enabled: false });
  });
  it("sletting er mykt og kan angres (deleteRule/restoreRule)", async () => {
    const { s } = await loaded();
    const id = s.state.rules[0].id;
    expect(await s.remove(id)).toBe(true);
    expect(s.state.rules.some((r) => r.id === id)).toBe(false);
    expect(s.state.note).toEqual({ kind: "deleted", id, name: "Gamle skjermbilder" });
    expect(await s.undoRemove()).toBe(true);
    expect(s.state.rules.some((r) => r.id === id)).toBe(true);
    expect(s.state.note?.kind).toBe("restored");
    expect(await s.undoRemove()).toBe(false);
  });
  it("feil fra bridgen gir feilkode og lar listen stå", async () => {
    const { b, s } = await loaded();
    const id = s.state.rules[0].id;
    await b.deleteRule(id);
    expect(await s.toggle(id, false)).toBe(false);
    expect(s.state.error?.code).toBe("rule_not_found");
    expect(s.state.rules.some((r) => r.id === id)).toBe(false);
  });
});

describe("regelbygger: forhåndsvisning før lagring", () => {
  it("kan ikke lagres før forhåndsvisning, og utdateres når vilkårene endres", async () => {
    const b = new MockBridge();
    let e = editorFor(null);
    e = { ...e, name: "Små filer", rows: [{ ...newRow("smaller_than_bytes"), text: "500", unit: "kb" }] };
    expect(canSave(e)).toBe(false);
    const blocked = await saveEditor(b, e);
    expect(blocked.id).toBeNull();
    expect((await b.listRules()).length).toBe(2);

    e = await runPreview(b, e);
    expect(e.error).toBeNull();
    expect(e.preview?.hits).toBeGreaterThan(0);
    expect(e.preview?.sample.length).toBeGreaterThan(0);
    expect(previewIsFresh(e)).toBe(true);
    expect((await b.listRules()).length).toBe(2); // forhåndsvisning lagrer ingenting

    e = { ...e, rows: [{ ...e.rows[0], text: "600" }] };
    expect(previewIsFresh(e)).toBe(false);
    expect(canSave(e)).toBe(false);

    e = await runPreview(b, e);
    const saved = await saveEditor(b, e);
    expect(saved.id).not.toBeNull();
    expect((await b.listRules()).map((r) => r.name)).toContain("Små filer");
  });
  it("valideringsfeil kommer som feilkode fra bridgen", async () => {
    const b = new MockBridge();
    const empty = await runPreview(b, { ...editorFor(null), rows: [] });
    expect(empty.error?.code).toBe("empty_rule");
    const bad = await runPreview(b, { ...editorFor(null), rows: [newRow("older_than_days")] });
    expect(bad.error?.code).toBe("invalid_condition");
    expect(canSave(bad)).toBe(false);

    let e = await runPreview(b, { ...editorFor(null), rows: [newRow("is_blurry")] });
    expect(e.error).toBeNull();
    const noName = await saveEditor(b, e);
    expect(noName.id).toBeNull();
    expect(noName.state.error?.code).toBe("empty_name");
    e = { ...e, name: "x".repeat(81) };
    expect((await saveEditor(b, e)).state.error?.code).toBe("name_too_long");
  });
  it("redigering beholder id og lagrer nytt navn og vilkår", async () => {
    const b = new MockBridge();
    const [first] = await b.listRules();
    let e = editorFor(first);
    expect(e.id).toBe(first.id);
    expect(conditionsOf(e.rows)).toEqual(first.conditions);
    e = await runPreview(b, { ...e, name: "Nytt navn" });
    const r = await saveEditor(b, e);
    expect(r.id).toBe(first.id);
    expect((await b.listRules()).find((x) => x.id === first.id)?.name).toBe("Nytt navn");
  });
  it("regelen foreslår bare: lagring lager kort, aldri papirkurv", async () => {
    const b = new MockBridge();
    const before = (await b.getState()).trashCount;
    let e = { ...editorFor(null), name: "Alle uskarpe", rows: [newRow("is_blurry")] };
    e = await runPreview(b, e);
    await saveEditor(b, e);
    expect((await b.getState()).trashCount).toBe(before);
  });
});

describe("sensitivitet (SensitivitySession)", () => {
  it("leser innstillinger og terskler, og lagrer valg per type", async () => {
    const b = new MockBridge();
    const s = new SensitivitySession(b);
    await s.load();
    expect(s.state.phase).toBe("ready");
    expect(s.state.thresholds?.visualMaxHamming).toBe(6);
    await s.choose("visual_dup", "high");
    expect(s.state).toMatchObject({ saved: true, saving: false, saveFailed: false });
    expect(s.state.settings?.sensitivity.visual_dup).toBe("high");
    expect(s.state.thresholds?.visualMaxHamming).toBe(8);
    expect((await b.getSuggestionSettings()).sensitivity.visual_dup).toBe("high");
    expect((await b.getSuggestionSettings()).sensitivity.blurry).toBe("normal");
  });
  it("samme verdi gir ingen kall, og de tre typene har kontroll", async () => {
    const b = new MockBridge();
    const s = new SensitivitySession(b);
    await s.load();
    await s.choose("blurry", "normal");
    expect(s.state.saved).toBe(false);
    expect(SENSITIVE_KINDS).toEqual(["visual_dup", "blurry", "series"]);
  });
});

describe("beskyttelser (ProtectionsSession)", () => {
  it("fjerner først etter bekreftelse, og kan angre", async () => {
    const b = new MockBridge();
    await b.protect({ scope: "album", target: "1" }, true);
    const s = new ProtectionsSession(b);
    await s.load();
    expect(s.state.items).toHaveLength(1);
    s.askRemove(s.state.items[0].scope);
    expect(s.state.items).toHaveLength(1); // ingenting endret ennå
    s.cancelRemove();
    expect((await b.listProtections())).toHaveLength(1);

    s.askRemove(s.state.items[0].scope);
    expect(await s.confirmRemove()).toBe(true);
    expect(s.state.items).toHaveLength(0);
    expect(s.state.note?.kind).toBe("removed");
    expect(await s.undoRemove()).toBe(true);
    expect(s.state.items).toHaveLength(1);
    expect(s.state.note?.kind).toBe("restored");
  });
  it("legger til mappebeskyttelse; tom sti avvises uten kall", async () => {
    const b = new MockBridge();
    const s = new ProtectionsSession(b);
    await s.load();
    expect(await s.addFolder("   ")).toMatchObject({ ok: false, error: { code: "empty_folder" } });
    expect(await s.addFolder(" D:\\Bilder\\Ferie\\ ")).toEqual({ ok: true });
    expect(s.state.items[0].scope).toEqual({ scope: "folder", target: "D:\\Bilder\\Ferie" });
    expect(describeTarget(s.state.items[0].scope, t)).toBe("D:\\Bilder\\Ferie");
    expect(describeTarget({ scope: "album", target: "2" }, t)).toBe("nr. 2");
  });
});
