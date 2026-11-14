import { describe, expect, it } from "vitest";
import { formatHash, NAV_ROUTES, navFor, navHash, parseHash, type Route } from "./router";

describe("router", () => {
  it("ukjent og tom hash gir Hjem", () => {
    for (const h of ["", "#", "#/", "#/finnes-ikke", "#/group/abc", "#/group/0"]) expect(parseHash(h)).toEqual({ name: "home" });
  });
  it("rundtur parse(format(r)) = r", () => {
    const routes: Route[] = [
      { name: "onboarding" }, { name: "home" }, { name: "review", kind: null }, { name: "review", kind: "blurry" },
      { name: "group", id: 7 }, { name: "cards", kind: "screenshot" }, { name: "cards", kind: null },
      { name: "trash" }, { name: "settings" }, { name: "people" }, { name: "rules" }, { name: "series", id: 42 },
      { name: "review", kind: "series" }, { name: "cards", kind: "rule" },
    ];
    for (const r of routes) expect(parseHash(formatHash(r))).toEqual(r);
  });
  it("ugyldig kategori gir null", () => {
    expect(parseHash("#/review/nope")).toEqual({ name: "review", kind: null });
  });
  it("ugyldig serie-id gir Hjem", () => {
    for (const h of ["#/series", "#/series/abc", "#/series/0", "#/series/-3"]) expect(parseHash(h)).toEqual({ name: "home" });
  });
  it("Personer og Regler er toppnivå; Serie hører til Gjennomgang", () => {
    expect(NAV_ROUTES).toEqual(["home", "review", "people", "rules", "trash", "settings"]);
    expect(navFor({ name: "people" })).toBe("people");
    expect(navFor({ name: "rules" })).toBe("rules");
    expect(navFor({ name: "series", id: 3 })).toBe("review");
    expect(navHash("people")).toBe("#/people");
  });
  it("Gruppe og Kort hører til Gjennomgang i navigasjonen", () => {
    expect(navFor({ name: "group", id: 1 })).toBe("review");
    expect(navFor({ name: "cards", kind: null })).toBe("review");
    expect(navFor({ name: "onboarding" })).toBeNull();
  });
});
