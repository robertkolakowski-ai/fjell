import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { SettingsSession } from "./settingsFlow";
import { initialSettings, LANGUAGE_CHOICES, reduceSettings, trashDayOptions } from "./settingsState";

async function loaded(opts = {}) {
  const b = new MockBridge(opts);
  const s = new SettingsSession(b);
  await s.load();
  return { b, s };
}

describe("innstillinger", () => {
  it("viser frist, karantenemappe og Tøm-standard fra bridge", async () => {
    const { s } = await loaded({ library: "D:\\Bilder" });
    expect(s.state.phase).toBe("ready");
    expect(s.state.settings).toEqual({ trashDays: 30, emptyDefault: "recycle", quarantineDir: "D:\\Bilder\\.fjell-karantene" });
  });
  it("uten mappe finnes ingen karantenemappe ennå", async () => {
    const { s } = await loaded({ library: null });
    expect(s.state.settings?.quarantineDir).toBeNull();
  });
  it("Tøm-standard er alltid Papirkurv (aldri permanent)", async () => {
    const { s } = await loaded();
    expect(s.state.settings?.emptyDefault).toBe("recycle");
  });
  it("lagrer ny frist og viser den i state, også i bridge", async () => {
    const { b, s } = await loaded();
    await s.setTrashDays(7);
    expect(s.state).toMatchObject({ saved: true, saving: false, saveFailed: false });
    expect(s.state.settings?.trashDays).toBe(7);
    expect((await b.getState()).trashDays).toBe(7);
  });
  it("ny frist gjelder bilder som flyttes etterpå", async () => {
    const { b, s } = await loaded();
    await s.setTrashDays(14);
    await b.startScan(false);
    const id = (await b.listCards("blurry", 1)).cards[0].id;
    await b.decide([id], "trash");
    const [item] = await b.listTrashed();
    expect(item.expiresAt - item.trashedAt).toBe(14 * 86_400);
  });
  it("samme verdi gir ingen kall", async () => {
    const { b, s } = await loaded();
    let calls = 0;
    const orig = b.setTrashDays.bind(b);
    b.setTrashDays = (d) => { calls++; return orig(d); };
    await s.setTrashDays(30);
    expect(calls).toBe(0);
  });
  it("feil: verdien er uendret og feilmelding settes", async () => {
    const { b, s } = await loaded();
    b.setTrashDays = () => Promise.reject({ code: "core_error", detail: "" });
    await s.setTrashDays(90);
    expect(s.state).toMatchObject({ saveFailed: true, saved: false, saving: false });
    expect(s.state.settings?.trashDays).toBe(30);
  });
  it("ugyldig frist avvises av bridge", async () => {
    const b = new MockBridge();
    await expect(b.setTrashDays(0)).rejects.toMatchObject({ code: "bad_request" });
    await expect(b.setTrashDays(366)).rejects.toMatchObject({ code: "bad_request" });
    await expect(b.setTrashDays(1.5)).rejects.toMatchObject({ code: "bad_request" });
  });
  it("feil ved lasting gir error og reload starter på nytt", () => {
    const err = reduceSettings(initialSettings, { type: "failed", error: { code: "core_error", detail: "" } });
    expect(err.phase).toBe("error");
    expect(reduceSettings(err, { type: "reload" })).toEqual(initialSettings);
  });
  it("valgene: gjeldende verdi tas med, sortert, uten duplikater", () => {
    expect(trashDayOptions(30)).toEqual([7, 14, 30, 60, 90]);
    expect(trashDayOptions(45)).toEqual([7, 14, 30, 45, 60, 90]);
    expect(trashDayOptions(3)[0]).toBe(3);
  });
  it("språkvalg: bare system, nb og en (pseudo er ikke et brukervalg)", () => {
    expect([...LANGUAGE_CHOICES]).toEqual(["system", "nb", "en"]);
  });
});
