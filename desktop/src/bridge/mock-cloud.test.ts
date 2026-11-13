// MockBridge speiler skallets skymappe-regler og språkmetoder.
import { describe, expect, it } from "vitest";
import { MockBridge } from "./mock-bridge";

describe("MockBridge: skymappe", () => {
  it("cloudSync følger bibliotekssti (også etter setLibrary)", async () => {
    const m = new MockBridge({ library: "C:\\Users\\a\\OneDrive - Firma AS\\Bilder" });
    expect((await m.getState()).cloudSync).toBe("onedrive");
    await m.setLibrary("D:\\Bilder");
    expect((await m.getState()).cloudSync).toBeNull();
    await m.setLibrary("/Users/a/iCloud Drive/Bilder");
    expect((await m.getState()).cloudSync).toBe("icloud");
  });
  it("papirkurv nektes med cloud_synced_readonly FØR noe endres, behold virker", async () => {
    const m = new MockBridge({ library: "/u/Dropbox/Bilder" });
    await m.startScan(true);
    const before = await m.getState();
    await expect(m.decide([1, 2], "trash")).rejects.toEqual({ code: "cloud_synced_readonly", detail: "dropbox" });
    const after = await m.getState();
    expect(after.trashCount).toBe(0);
    expect(after.perKind).toEqual(before.perKind);
    await expect(m.undo()).rejects.toMatchObject({ code: "nothing_to_undo" });
    await m.decide([1], "keep");
    expect((await m.getState()).perKind.reduce((s, k) => s + k.count, 0)).toBe(before.perKind.reduce((s, k) => s + k.count, 0) - 1);
  });
  it("lokalt bibliotek: papirkurv virker som før", async () => {
    const m = new MockBridge();
    await m.startScan(true);
    await m.decide([1], "trash");
    expect((await m.getState()).trashCount).toBe(1);
  });
});

describe("MockBridge: språk", () => {
  it("null som standard, lagrer system/nb/en, avviser pseudo og ukjent", async () => {
    const m = new MockBridge();
    expect(await m.getLanguage()).toBeNull();
    expect(await m.setLanguage("en")).toBe("en");
    expect(await m.getLanguage()).toBe("en");
    await expect(m.setLanguage("pseudo")).rejects.toMatchObject({ code: "bad_request" });
    await expect(m.setLanguage("xx")).rejects.toMatchObject({ code: "bad_request" });
    expect(await m.getLanguage()).toBe("en");
    expect(await new MockBridge({ language: "nb" }).getLanguage()).toBe("nb");
  });
});

describe("MockBridge: flere mapper (kontrakt for bølge 2)", () => {
  it("legg til, list og fjern; overlapp og første mappe avvises", async () => {
    const m = new MockBridge({ library: "D:\\Bilder" });
    expect((await m.getState()).roots).toEqual([
      { key: 1, path: "D:\\Bilder", primary: true, cloudSync: null, quarantineDir: "D:\\Bilder\\.fjell-karantene" },
    ]);
    const two = await m.addLibraryRoot("E:\\OneDrive\\Foto");
    expect(two.map((r) => [r.key, r.primary, r.cloudSync])).toEqual([[1, true, null], [2, false, "onedrive"]]);
    expect(two[0].online).toBe(true);
    // Én skysynket mappe blant flere sperrer ikke hele biblioteket.
    expect((await m.getState()).cloudSync).toBeNull();
    await expect(m.addLibraryRoot("D:\\Bilder\\Ferie")).rejects.toMatchObject({ code: "root_overlap" });
    await expect(m.addLibraryRoot("d:/bilder/")).rejects.toMatchObject({ code: "root_overlap" });
    await expect(m.addLibraryRoot("D:\\")).rejects.toMatchObject({ code: "root_overlap" });
    await expect(m.removeLibraryRoot(1)).rejects.toMatchObject({ code: "root_is_primary" });
    await expect(m.removeLibraryRoot(9)).rejects.toMatchObject({ code: "not_found" });
    expect((await m.removeLibraryRoot(2)).length).toBe(1);
    // Nøkler gjenbrukes ikke, og setLibrary erstatter med én mappe.
    expect((await m.addLibraryRoot("F:\\Arkiv"))[1].key).toBe(3);
    await m.setLibrary("G:\\Nytt");
    expect((await m.listLibraryRoots()).map((r) => r.path)).toEqual(["G:\\Nytt"]);
  });
});
