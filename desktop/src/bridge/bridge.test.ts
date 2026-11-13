import { describe, expect, it, vi } from "vitest";
import type { CoreBridge } from "./core-bridge";
import { MockBridge } from "./mock-bridge";
import { TauriBridge, type UpdateHandle, type UpdaterApi } from "./tauri-bridge";
import { groupCards } from "./groups";

// Type-test: begge klassene må være gjensidig tilordnebare med CoreBridge og ha samme nøkler.
type Keys<T> = keyof T;
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
const _k1: Same<Keys<MockBridge>, Keys<TauriBridge>> = true;
const _m: CoreBridge = new MockBridge();
const _t: CoreBridge = new TauriBridge(async () => undefined as never);
void _k1; void _m; void _t;

describe("identisk grensesnitt", () => {
  it("samme offentlige metoder", () => {
    const names = (o: object) =>
      Object.getOwnPropertyNames(Object.getPrototypeOf(o)).filter((n) => n !== "constructor" && !n.startsWith("wait")).sort();
    const pub = [
      "addLibraryRoot", "cancelScan", "checkForUpdate", "completeOnboarding", "decide", "emptyTrash", "getAppVersion", "getAutoUpdate",
      "getFirstRun", "getLanguage", "getLibrary", "getSettings", "getState", "getThumbnail", "getUpdateDownloadUrl", "installUpdate", "listCards", "listLibraryRoots", "listTrashed",
      "pauseScan", "pickFolder", "purge", "removeLibraryRoot", "restore", "restoreAll", "resumeScan", "setAutoUpdate", "setKeeper", "setLanguage",
      "setLibrary", "setTrashDays", "startScan", "undo",
    ];
    expect(names(new TauriBridge())).toEqual(pub);
    expect(names(new MockBridge()).filter((n) => pub.includes(n))).toEqual(pub);
  });
});

describe("TauriBridge", () => {
  it("kaller eksisterende kommandoer med riktige argumenter", async () => {
    const call = vi.fn().mockResolvedValue(7);
    const b = new TauriBridge(call);
    await b.startScan(true);
    await b.listCards("blurry", 20, 3);
    await b.decide([1, 2], "trash");
    await b.undo();
    await b.getState();
    expect(call.mock.calls).toEqual([
      ["start_scan", { full: true }],
      ["list_cards", { kind: "blurry", limit: 20, cursor: 3 }],
      ["decide_cards", { ids: [1, 2], decision: "trash" }],
      ["undo"],
      ["get_state"],
    ]);
  });
  it("setKeeper kaller set_keeper", async () => {
    const call = vi.fn().mockResolvedValue(undefined);
    await new TauriBridge(call).setKeeper(4, 9);
    expect(call.mock.calls).toEqual([["set_keeper", { group: 4, keeper: 9 }]]);
  });
  it("purge tømmer via empty_trash og gir antall", async () => {
    const call = vi.fn().mockResolvedValue({ purged: 3, keptTrashed: 0 });
    expect(await new TauriBridge(call).purge()).toBe(3);
    expect(call.mock.calls).toEqual([["empty_trash"]]);
  });
});

describe("mappevalg (pickFolder)", () => {
  it("TauriBridge: ingen invoke-kommando; delegerer til plugin-funksjonen med tittel, null = avbrutt", async () => {
    const call = vi.fn();
    const pick = vi.fn().mockResolvedValueOnce("D:\\Ferie").mockResolvedValueOnce(null);
    const b = new TauriBridge(call, undefined, pick);
    expect(b.canPickFolder).toBe(true);
    expect(await b.pickFolder("Velg")).toBe("D:\\Ferie");
    expect(await b.pickFolder("Velg")).toBeNull();
    expect(pick.mock.calls).toEqual([["Velg"], ["Velg"]]);
    expect(call).not.toHaveBeenCalled();
  });
  it("MockBridge: sti, avbrutt og utilgjengelig (standard) er tre ulike utfall", async () => {
    expect(await new MockBridge({ pick: "C:\\Bilder\\Ferie" }).pickFolder()).toBe("C:\\Bilder\\Ferie");
    expect(await new MockBridge({ pick: null }).pickFolder()).toBeNull();
    const off = new MockBridge();
    expect(off.canPickFolder).toBe(false);
    await expect(off.pickFolder()).rejects.toMatchObject({ code: "unsupported" });
  });
  it("pickFolder setter ikke biblioteket", async () => {
    const b = new MockBridge({ pick: "D:\\Annet", library: "C:\\Bilder" });
    await b.pickFolder();
    expect(await b.getLibrary()).toBe("C:\\Bilder");
  });
});

describe("MockBridge", () => {
  it("er deterministisk", async () => {
    const a = await new MockBridge().listCards("exact_dup", 50);
    const b = await new MockBridge().listCards("exact_dup", 50);
    expect(a).toEqual(b);
  });
  it("skann → papirkurv → angre → purge", async () => {
    const m = new MockBridge();
    expect((await m.getState()).reclaimableBytes).toBe(0);
    await m.startScan(false);
    const s0 = await m.getState();
    expect(s0.scanPhase).toBe("done");
    const page = await m.listCards("blurry", 2);
    expect(page.cards).toHaveLength(2);
    expect(page.next).not.toBeNull();
    const ids = page.cards.map((c) => c.id);
    await m.decide(ids, "trash");
    expect((await m.getState()).trashCount).toBe(2);
    await m.undo();
    const s2 = await m.getState();
    expect(s2.trashCount).toBe(0);
    expect(s2.reclaimableBytes).toBe(s0.reclaimableBytes);
    await m.decide(ids, "trash");
    expect(await m.purge()).toBe(2);
    expect((await m.getState()).freedBytes).toBeGreaterThan(0);
  });
  it("feil: ukjent kort og ingenting å angre", async () => {
    const m = new MockBridge();
    await expect(m.decide([9999], "trash")).rejects.toMatchObject({ code: "unknown_card" });
    await expect(m.undo()).rejects.toMatchObject({ code: "nothing_to_undo" });
  });
  it("forsinkelse kan slås på", async () => {
    vi.useFakeTimers();
    const p = new MockBridge({ delayMs: 100 }).getState();
    let done = false;
    void p.then(() => (done = true));
    await vi.advanceTimersByTimeAsync(99);
    expect(done).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(done).toBe(true);
    vi.useRealTimers();
  });
  it("grupperer kort", async () => {
    const m = new MockBridge();
    const g = groupCards((await m.listCards("exact_dup", 50)).cards);
    expect(g).toHaveLength(3);
    expect(g[0].cards).toHaveLength(2);
  });
});

describe("TauriBridge: M7-hull", () => {
  it("kaller riktige kommandoer med riktige argumenter", async () => {
    const call = vi.fn().mockResolvedValue(undefined);
    const b = new TauriBridge(call);
    await b.listTrashed();
    await b.restore([4, 5]);
    await b.restoreAll();
    await b.pauseScan();
    await b.resumeScan();
    await b.cancelScan();
    await b.getLibrary();
    await b.setLibrary("D:\\Bilder");
    await b.getFirstRun();
    await b.completeOnboarding();
    await b.getSettings();
    await b.setTrashDays(14);
    expect(call.mock.calls).toEqual([
      ["list_trashed"], ["restore_trashed", { ids: [4, 5] }], ["restore_all_trashed"], ["pause_scan"], ["resume_scan"],
      ["cancel_scan"], ["get_library"], ["set_library", { path: "D:\\Bilder" }], ["get_first_run"], ["complete_onboarding"],
      ["get_settings"], ["set_trash_days", { days: 14 }],
    ]);
  });
});

describe("TauriBridge: språk", () => {
  it("kaller get_language og set_language med riktige argumenter", async () => {
    const call = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce("nb");
    const b = new TauriBridge(call);
    expect(await b.getLanguage()).toBeNull();
    expect(await b.setLanguage("nb")).toBe("nb");
    expect(call.mock.calls).toEqual([["get_language"], ["set_language", { language: "nb" }]]);
  });
});

describe("MockBridge: papirkurvliste og gjenoppretting", () => {
  it("restore tar bildet ut av listen, uten å lage nytt forslag, og angre treffer ikke gjenopprettede", async () => {
    const m = new MockBridge();
    await m.startScan(false);
    const ids = (await m.listCards("blurry", 3)).cards.map((c) => c.id);
    await m.decide(ids, "trash");
    const [a, ...rest] = await m.listTrashed();
    expect(rest).toHaveLength(2);
    expect(await m.restore([a.asset, 99999])).toEqual({ restored: 1 });
    expect((await m.listTrashed()).map((i) => i.asset)).toEqual(rest.map((i) => i.asset));
    expect((await m.getState()).trashCount).toBe(2);
    expect(await m.restore([a.asset])).toEqual({ restored: 0 });
    // Angre tar de to som ligger igjen (batchen er justert), ikke det gjenopprettede.
    await m.undo();
    expect((await m.getState()).trashCount).toBe(0);
  });
  it("restoreAll tømmer listen", async () => {
    const m = new MockBridge();
    await m.startScan(false);
    await m.decide((await m.listCards("screenshot", 4)).cards.map((c) => c.id), "trash");
    expect(await m.restoreAll()).toEqual({ restored: 4 });
    expect(await m.listTrashed()).toEqual([]);
    expect(await m.restoreAll()).toEqual({ restored: 0 });
  });
  it("utløpt vises i state og liste, men bildene finnes fortsatt (aldri stille slettet)", async () => {
    let t = 0;
    const m = new MockBridge({ now: () => t, trashDays: 1 });
    await m.startScan(false);
    await m.decide([(await m.listCards("blurry", 1)).cards[0].id], "trash");
    t = 2 * 86_400_000;
    expect((await m.getState()).trashExpired).toBe(1);
    expect((await m.listTrashed())[0].expired).toBe(true);
    expect((await m.getState()).trashCount).toBe(1);
  });
  it("tøm fjerner fra listen", async () => {
    const m = new MockBridge();
    await m.startScan(false);
    await m.decide([(await m.listCards("blurry", 1)).cards[0].id], "trash");
    await m.emptyTrash();
    expect(await m.listTrashed()).toEqual([]);
  });
});

// ---- Beta-kanal: oppdatering (M9) ----
function updater(over: Partial<UpdaterApi> = {}): UpdaterApi & { relaunch: ReturnType<typeof vi.fn> } {
  return { available: () => true, check: async () => null, getVersion: async () => "0.1.7", ...over, relaunch: vi.fn().mockResolvedValue(undefined) } as never;
}
const handle = (events: { event: string; data?: { contentLength?: number; chunkLength?: number } }[], fail = false): UpdateHandle => ({
  version: "0.1.8", date: "2026-11-12T10:00:00Z", body: "Notater",
  downloadAndInstall: async (cb) => { for (const e of events) cb(e); if (fail) throw new Error("signatur"); },
});

describe("TauriBridge: oppdatering", () => {
  it("auto-valget går via get_auto_update/set_auto_update", async () => {
    const call = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    const b = new TauriBridge(call, updater());
    expect(await b.getAutoUpdate()).toBe(false);
    expect(await b.setAutoUpdate(true)).toBe(true);
    expect(call.mock.calls).toEqual([["get_auto_update"], ["set_auto_update", { on: true }]]);
  });
  it("utenfor Tauri: versjon «dev» og forklarende feil ved sjekk (ingen plugin kalles)", async () => {
    const check = vi.fn();
    const b = new TauriBridge(vi.fn(), updater({ available: () => false, check }));
    expect(await b.getAppVersion()).toBe("dev");
    await expect(b.checkForUpdate()).rejects.toMatchObject({ code: "unsupported" });
    expect(check).not.toHaveBeenCalled();
  });
  it("versjon fra getVersion", async () => {
    expect(await new TauriBridge(vi.fn(), updater()).getAppVersion()).toBe("0.1.7");
  });
  it("ingen ny versjon", async () => {
    expect(await new TauriBridge(vi.fn(), updater()).checkForUpdate()).toEqual({ available: false });
  });
  it("ny versjon gir versjon, notater og dato", async () => {
    const b = new TauriBridge(vi.fn(), updater({ check: async () => handle([]) }));
    expect(await b.checkForUpdate()).toEqual({ available: true, version: "0.1.8", notes: "Notater", date: "2026-11-12T10:00:00Z" });
  });
  it("sjekk som feiler gir update_check_failed", async () => {
    const b = new TauriBridge(vi.fn(), updater({ check: async () => { throw new Error("nett"); } }));
    await expect(b.checkForUpdate()).rejects.toMatchObject({ code: "update_check_failed" });
  });
  it("installasjon gir fremdrift, og starter på nytt etterpå", async () => {
    const u = updater({ check: async () => handle([{ event: "Started", data: { contentLength: 100 } }, { event: "Progress", data: { chunkLength: 40 } }, { event: "Progress", data: { chunkLength: 60 } }, { event: "Finished" }]) });
    const b = new TauriBridge(vi.fn(), u);
    await b.checkForUpdate();
    const seen: [number, number | null][] = [];
    await b.installUpdate((d, t) => seen.push([d, t]));
    expect(seen).toEqual([[0, 100], [40, 100], [100, 100], [100, 100]]);
    expect(u.relaunch).toHaveBeenCalledTimes(1);
  });
  it("ukjent størrelse gir total null", async () => {
    const b = new TauriBridge(vi.fn(), updater({ check: async () => handle([{ event: "Started", data: {} }, { event: "Progress", data: { chunkLength: 5 } }]) }));
    await b.checkForUpdate();
    const seen: [number, number | null][] = [];
    await b.installUpdate((d, t) => seen.push([d, t]));
    expect(seen).toEqual([[0, null], [5, null]]);
  });
  it("installasjon som feiler starter ikke på nytt", async () => {
    const u = updater({ check: async () => handle([], true) });
    const b = new TauriBridge(vi.fn(), u);
    await b.checkForUpdate();
    await expect(b.installUpdate(() => undefined)).rejects.toMatchObject({ code: "update_install_failed" });
    expect(u.relaunch).not.toHaveBeenCalled();
  });
  it("installasjon som stopper etter nedlasting (appen lever fortsatt) gir update_install_stalled", async () => {
    vi.useFakeTimers();
    try {
      const hang: UpdateHandle = { version: "0.1.8", downloadAndInstall: (cb) => { cb({ event: "Finished" }); return new Promise(() => undefined); } };
      const u = updater({ check: async () => hang });
      const b = new TauriBridge(vi.fn(), u, undefined, 5000);
      await b.checkForUpdate();
      const p = b.installUpdate(() => undefined);
      const seen = expect(p).rejects.toMatchObject({ code: "update_install_stalled" });
      await vi.advanceTimersByTimeAsync(5000);
      await seen;
      expect(u.relaunch).not.toHaveBeenCalled();
      // Kan prøves igjen: funnet oppdatering er beholdt.
      const again = b.installUpdate(() => undefined);
      const seen2 = expect(again).rejects.toMatchObject({ code: "update_install_stalled" });
      await vi.advanceTimersByTimeAsync(5000);
      await seen2;
    } finally { vi.useRealTimers(); }
  });
  it("ingen stopp-klokke før nedlastingen er ferdig (treg nedlasting er ikke en feil)", async () => {
    vi.useFakeTimers();
    try {
      let finish: () => void = () => undefined;
      const slow: UpdateHandle = { version: "0.1.8", downloadAndInstall: (cb) => { cb({ event: "Started", data: {} }); return new Promise<void>((r) => { finish = r; }); } };
      const u = updater({ check: async () => slow });
      const b = new TauriBridge(vi.fn(), u, undefined, 5000);
      await b.checkForUpdate();
      const p = b.installUpdate(() => undefined);
      await vi.advanceTimersByTimeAsync(60_000);
      finish();
      await p;
      expect(u.relaunch).toHaveBeenCalledTimes(1);
    } finally { vi.useRealTimers(); }
  });
  it("adressen for manuell nedlasting kommer fra skallet (get_update_download_url), ikke fra frontend", async () => {
    const call = vi.fn().mockResolvedValueOnce("adresse-fra-skallet");
    expect(await new TauriBridge(call, updater()).getUpdateDownloadUrl()).toBe("adresse-fra-skallet");
    expect(call.mock.calls).toEqual([["get_update_download_url"]]);
  });
  it("installasjon uten forutgående sjekk gjør ingenting", async () => {
    const u = updater();
    await expect(new TauriBridge(vi.fn(), u).installUpdate(() => undefined)).rejects.toMatchObject({ code: "no_update" });
    expect(u.relaunch).not.toHaveBeenCalled();
  });
});

describe("MockBridge: oppdatering", () => {
  it("none, available og error", async () => {
    expect(await new MockBridge().checkForUpdate()).toEqual({ available: false });
    expect(await new MockBridge({ update: "available" }).checkForUpdate()).toMatchObject({ available: true, version: "0.1.99" });
    await expect(new MockBridge({ update: "error" }).checkForUpdate()).rejects.toMatchObject({ code: "update_check_failed" });
  });
  it("installasjon gir fremdrift fra 0 til total", async () => {
    const seen: number[] = [];
    await new MockBridge({ update: "available" }).installUpdate((d) => seen.push(d));
    expect(seen[0]).toBe(0);
    expect(seen[seen.length - 1]).toBe(24_000_000);
  });
  it("auto er av som standard og kan slås på", async () => {
    const b = new MockBridge();
    expect(await b.getAutoUpdate()).toBe(false);
    expect(await b.setAutoUpdate(true)).toBe(true);
    expect(await b.getAutoUpdate()).toBe(true);
  });
});
