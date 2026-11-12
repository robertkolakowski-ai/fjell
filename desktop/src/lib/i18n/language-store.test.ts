import { beforeEach, describe, expect, it, vi } from "vitest";
import { _resetLanguageStoreForTest, getLanguage, initLanguage, setLanguage, t } from "./index";

const KEY = "fjell.language";
const mem = new Map<string, string>();

function store(saved: string | null | Error) {
  return {
    getLanguage: vi.fn(async () => {
      if (saved instanceof Error) throw saved;
      return saved;
    }),
    setLanguage: vi.fn(async (l: string) => l),
  };
}

beforeEach(() => {
  mem.clear();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => mem.get(k) ?? null,
    setItem: (k: string, v: string) => void mem.set(k, v),
  });
  _resetLanguageStoreForTest();
});

describe("initLanguage", () => {
  it("lagret verdi i skallet vinner over localStorage og migrerer ikke", async () => {
    mem.set(KEY, "en");
    const s = store("nb");
    expect(await initLanguage(s, ["en-US"])).toBe("nb");
    expect(getLanguage()).toBe("nb");
    expect(t("common.count.photos", { count: 2 })).toBe("2 bilder");
    expect(s.setLanguage).not.toHaveBeenCalled();
  });
  it("None => behold localStorage-verdien og migrer den med set_language", async () => {
    mem.set(KEY, "en");
    const s = store(null);
    await initLanguage(s, ["nb-NO"]);
    expect(s.setLanguage).toHaveBeenCalledExactlyOnceWith("en");
  });
  it("None og ingenting lagret lokalt => ingen migrering", async () => {
    const s = store(null);
    await initLanguage(s, ["nb-NO"]);
    expect(s.setLanguage).not.toHaveBeenCalled();
  });
  it("pseudo migreres aldri", async () => {
    mem.set(KEY, "pseudo");
    const s = store(null);
    await initLanguage(s);
    expect(s.setLanguage).not.toHaveBeenCalled();
  });
  it("ukjent verdi fra skallet regnes som ikke valgt", async () => {
    mem.set(KEY, "nb");
    const s = store("fr");
    await initLanguage(s);
    expect(s.setLanguage).toHaveBeenCalledExactlyOnceWith("nb");
  });
  it("feil fra skallet gir reserve (localStorage/system) uten å kaste", async () => {
    const s = store(new Error("ikke tilgjengelig"));
    await expect(initLanguage(s)).resolves.toBeDefined();
    expect(s.setLanguage).not.toHaveBeenCalled();
  });
  it("valg gjort mens skallet svarer, vinner over skallets verdi", async () => {
    const s = store("nb");
    const p = initLanguage(s);
    setLanguage("en");
    await p;
    expect(getLanguage()).toBe("en");
    expect(s.setLanguage).toHaveBeenLastCalledWith("en");
  });
});

describe("setLanguage", () => {
  it("lagrer i skallet og i localStorage; pseudo bare lokalt", async () => {
    const s = store("system");
    await initLanguage(s, ["en"]);
    setLanguage("nb");
    expect(s.setLanguage).toHaveBeenCalledExactlyOnceWith("nb");
    expect(mem.get(KEY)).toBe("nb");
    setLanguage("pseudo");
    expect(s.setLanguage).toHaveBeenCalledTimes(1);
    expect(mem.get(KEY)).toBe("pseudo");
  });
  it("uten skall: bare localStorage, ingen feil", () => {
    setLanguage("en");
    expect(mem.get(KEY)).toBe("en");
  });
  it("feil ved lagring i skallet kaster ikke og språket gjelder likevel", async () => {
    const s = store("nb");
    s.setLanguage.mockRejectedValue(new Error("disk full"));
    await initLanguage(s);
    expect(() => setLanguage("en")).not.toThrow();
    await Promise.resolve();
    expect(getLanguage()).toBe("en");
  });
});
