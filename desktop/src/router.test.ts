import { describe, expect, it } from "vitest";
import { formatHash, navFor, parseHash, type Route } from "./router";

describe("router", () => {
  it("ukjent og tom hash gir Hjem", () => {
    for (const h of ["", "#", "#/", "#/finnes-ikke", "#/group/abc", "#/group/0"]) expect(parseHash(h)).toEqual({ name: "home" });
  });
  it("rundtur parse(format(r)) = r", () => {
    const routes: Route[] = [
      { name: "onboarding" }, { name: "home" }, { name: "review", kind: null }, { name: "review", kind: "blurry" },
      { name: "group", id: 7 }, { name: "cards", kind: "screenshot" }, { name: "cards", kind: null },
      { name: "trash" }, { name: "settings" },
    ];
    for (const r of routes) expect(parseHash(formatHash(r))).toEqual(r);
  });
  it("ugyldig kategori gir null", () => {
    expect(parseHash("#/review/nope")).toEqual({ name: "review", kind: null });
  });
  it("Gruppe og Kort hører til Gjennomgang i navigasjonen", () => {
    expect(navFor({ name: "group", id: 1 })).toBe("review");
    expect(navFor({ name: "cards", kind: null })).toBe("review");
    expect(navFor({ name: "onboarding" })).toBeNull();
  });
});
