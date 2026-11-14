import { describe, expect, test } from "vitest";
import { MockBridge, mockThumbSvg } from "./mock-bridge";
import { TauriBridge } from "./tauri-bridge";
import { isCancelled, thumbError, thumbFromDto, ThumbStore } from "./thumbs";
import type { Thumbnail } from "./types";

const T = (n: number): Thumbnail => ({ url: `data:image/jpeg;base64,AA${n}`, width: 4, height: 3 });
const tick = () => new Promise((r) => setTimeout(r, 0));

/** Hentefunksjon der testen styrer når hvert kall blir ferdig. */
function manual() {
  const calls: { id: number; ok(t: Thumbnail): void; fail(e: unknown): void }[] = [];
  const fetchOne = (id: number) => new Promise<Thumbnail>((ok, fail) => calls.push({ id, ok, fail }));
  return { calls, fetchOne };
}

describe("ThumbStore", () => {
  test("deduplisering: samtidige kall for samme id gir én henting", async () => {
    const m = manual();
    const s = new ThumbStore(m.fetchOne);
    const a = s.load(1);
    const b = s.load(1);
    await tick();
    expect(m.calls).toHaveLength(1);
    m.calls[0].ok(T(1));
    expect(await a.promise).toEqual(T(1));
    expect(await b.promise).toEqual(T(1));
  });

  test("maks 4 samtidige, resten står i kø og slippes inn etter hvert", async () => {
    const m = manual();
    const s = new ThumbStore(m.fetchOne);
    const hs = [1, 2, 3, 4, 5, 6].map((i) => s.load(i));
    await tick();
    expect(s.inFlight).toBe(4);
    expect(s.queued).toBe(2);
    m.calls[0].ok(T(1));
    await hs[0].promise;
    await tick();
    expect(m.calls.map((c) => c.id)).toEqual([1, 2, 3, 4, 5]);
    expect(s.inFlight).toBe(4);
  });

  test("avbryt i kø: hentingen starter aldri; avbryt med andre ventende beholder den", async () => {
    const m = manual();
    const s = new ThumbStore(m.fetchOne, { concurrency: 1 });
    s.load(1);
    const q = s.load(2);
    const shared1 = s.load(3);
    const shared2 = s.load(3);
    await tick();
    q.cancel();
    shared1.cancel();
    await expect(q.promise).rejects.toSatisfy(isCancelled);
    m.calls[0].ok(T(1));
    await tick();
    expect(m.calls.map((c) => c.id)).toEqual([1, 3]);
    m.calls[1].ok(T(3));
    expect(await shared2.promise).toEqual(T(3));
    await expect(shared1.promise).rejects.toSatisfy(isCancelled);
  });

  test("LRU: eldste kastes ved kapasitet, bruk oppfrisker", async () => {
    const s = new ThumbStore(async (id) => T(id), { capacity: 3 });
    for (const i of [1, 2, 3]) await s.load(i).promise;
    s.peek(1);
    await s.load(4).promise;
    expect(s.size).toBe(3);
    expect(s.peek(2)).toBeUndefined();
    expect(s.peek(1)).toBeDefined();
  });

  test("cache-treff henter ikke på nytt", async () => {
    let n = 0;
    const s = new ThumbStore(async (id) => { n++; return T(id); });
    await s.load(7).promise;
    await s.load(7).promise;
    expect(n).toBe(1);
  });

  test("feil caches og mappes til kontraktens koder", async () => {
    let n = 0;
    const s = new ThumbStore(async () => { n++; throw { code: "corrupt", detail: "x" }; });
    await expect(s.load(1).promise).rejects.toMatchObject({ code: "corrupt" });
    await expect(s.load(1).promise).rejects.toMatchObject({ code: "corrupt" });
    expect(n).toBe(1);
    s.invalidate(1);
    await expect(s.load(1).promise).rejects.toMatchObject({ code: "corrupt" });
    expect(n).toBe(2);
  });

  test("en feil stopper ikke køen", async () => {
    const m = manual();
    const s = new ThumbStore(m.fetchOne, { concurrency: 1 });
    const a = s.load(1);
    const b = s.load(2);
    await tick();
    m.calls[0].fail("rar feil");
    await expect(a.promise).rejects.toMatchObject({ code: "unavailable" });
    await tick();
    m.calls[1].ok(T(2));
    expect(await b.promise).toEqual(T(2));
  });
});

describe("feilmapping og DTO", () => {
  test("kjente koder beholdes, ukjente og ikke-objekter blir unavailable", () => {
    for (const code of ["unavailable", "cloud_placeholder", "corrupt", "not_found", "no_library"]) {
      expect(thumbError({ code, detail: "d" })).toEqual({ code, detail: "d" });
    }
    expect(thumbError({ code: "boom" }).code).toBe("unavailable");
    expect(thumbError("tekst").code).toBe("unavailable");
    expect(thumbError(null).code).toBe("unavailable");
  });

  test("ThumbDto blir data:-URL; annet enn JPEG/base64 avvises som corrupt", () => {
    expect(thumbFromDto({ mime: "image/jpeg", data: "QUJD", width: 256, height: 171 })).toEqual({ url: "data:image/jpeg;base64,QUJD", width: 256, height: 171 });
    expect(() => thumbFromDto({ mime: "text/html", data: "QUJD", width: 1, height: 1 })).toThrowError();
    expect(() => thumbFromDto({ mime: "image/jpeg", data: "a\"onerror=x", width: 1, height: 1 })).toThrowError();
  });
});

describe("TauriBridge.getThumbnail", () => {
  test("kaller get_thumbnail med argumentet id og returnerer data:-URL", async () => {
    const seen: unknown[] = [];
    const b = new TauriBridge(async (cmd, args) => { seen.push([cmd, args]); return { mime: "image/jpeg", data: "QUJD", width: 2, height: 3 } as never; });
    expect(await b.getThumbnail(42)).toEqual({ url: "data:image/jpeg;base64,QUJD", width: 2, height: 3 });
    expect(seen).toEqual([["get_thumbnail", { id: 42 }]]);
  });
  test("feil fra skallet blir typet BridgeError", async () => {
    const b = new TauriBridge(async () => { throw { code: "cloud_placeholder", detail: "sky" }; });
    await expect(b.getThumbnail(1)).rejects.toEqual({ code: "cloud_placeholder", detail: "sky" });
    const c = new TauriBridge(async () => { throw "kaos"; });
    await expect(c.getThumbnail(1)).rejects.toMatchObject({ code: "unavailable" });
  });
});

describe("MockBridge.getThumbnail", () => {
  test("deterministisk, varierer på id, stående hver femte", async () => {
    const b = new MockBridge();
    const a1 = await b.getThumbnail(1);
    expect(a1).toEqual(await b.getThumbnail(1));
    expect(a1.url).not.toBe((await b.getThumbnail(2)).url);
    expect(a1.url.startsWith("data:image/svg+xml")).toBe(true);
    expect(mockThumbSvg(5).height).toBeGreaterThan(mockThumbSvg(5).width);
    expect(a1.width).toBeGreaterThan(a1.height);
  });
  test("feil per id, ugyldig id og manglende bibliotek", async () => {
    const b = new MockBridge({ thumbErrors: { 3: "corrupt", 4: "cloud_placeholder" } });
    await expect(b.getThumbnail(3)).rejects.toMatchObject({ code: "corrupt" });
    await expect(b.getThumbnail(4)).rejects.toMatchObject({ code: "cloud_placeholder" });
    await expect(b.getThumbnail(0)).rejects.toMatchObject({ code: "not_found" });
    await expect(new MockBridge({ library: null }).getThumbnail(1)).rejects.toMatchObject({ code: "no_library" });
  });
});
