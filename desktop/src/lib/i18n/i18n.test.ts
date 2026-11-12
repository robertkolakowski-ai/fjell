import { beforeEach, expect, test } from "vitest";
import { fmt, reasonLabel, resolveSystemLocale, setLanguage, t, getLocale } from "./index";
import { formatMessage } from "./icu";
import { nb } from "./strings/nb";
import { en } from "./strings/en";
import { pseudo } from "./strings/pseudo";

beforeEach(() => setLanguage("nb"));

test("kataloger har samme nøkler", () => {
  expect(Object.keys(en).sort()).toEqual(Object.keys(nb).sort());
  expect(Object.keys(pseudo).sort()).toEqual(Object.keys(nb).sort());
});

test("plural og # i nb og en", () => {
  expect(t("review.bar.trash", { count: 1, size: "1,2 GB" })).toBe("Flytt 1 til papirkurv (1,2 GB)");
  expect(t("common.count.photos", { count: 1 })).toBe("1 bilde");
  expect(t("common.count.photos", { count: 1234 })).toMatch(/^1.234 bilder$/);
  setLanguage("en");
  expect(t("common.count.photos", { count: 1 })).toBe("1 photo");
  expect(t("common.count.photos", { count: 2 })).toBe("2 photos");
});

test("select og =N", () => {
  const ctx = { intl: "en" };
  expect(formatMessage("{g, select, a {A} other {X}}", { g: "a" }, ctx)).toBe("A");
  expect(formatMessage("{g, select, a {A} other {X}}", { g: "z" }, ctx)).toBe("X");
  expect(formatMessage("{n, plural, =0 {none} one {# x} other {# xs}}", { n: 0 }, ctx)).toBe("none");
});

test("manglende parameter synliggjøres", () => {
  const miss: string[] = [];
  expect(formatMessage("a {x} b", {}, { intl: "en" }, (n) => miss.push(n))).toBe("a {x} b");
  expect(miss).toEqual(["x"]);
});

test("pseudo: ramme, aksent, +30 %, ICU urørt", () => {
  setLanguage("pseudo");
  const s = t("home.ring.caption", { size: "5 GB" });
  expect(s.startsWith("[") && s.endsWith("]")).toBe(true);
  expect(s).toContain("5 GB");
  expect(s).toMatch(/[åæøü]/);
  expect(pseudo["home.ring.caption"].length).toBeGreaterThanOrEqual(Math.ceil(nb["home.ring.caption"].length * 1.2));
  expect(t("common.count.photos", { count: 2 })).toMatch(/^\[2 bild.r/);
});

test("systemspråk og overstyring", () => {
  expect(resolveSystemLocale(["nb-NO", "en"])).toBe("nb");
  expect(resolveSystemLocale(["nn-NO"])).toBe("nb");
  expect(resolveSystemLocale(["de-DE", "en-GB"])).toBe("en");
  expect(resolveSystemLocale(["de"])).toBe("en");
  setLanguage("system", ["en-US"]);
  expect(getLocale()).toBe("en");
  setLanguage("en");
  expect(t("common.cancel")).toBe("Cancel");
});

test("formatering følger språk", () => {
  expect(fmt.percent(0.82)).toBe("82 %");
  expect(fmt.bytes(1.2 * 1024 ** 3)).toBe("1,2 GB");
  setLanguage("en");
  expect(fmt.bytes(1.2 * 1024 ** 3)).toBe("1.2\u00a0GB");
});

test("begrunnelseskode: ukjent gir nøytral tekst", () => {
  expect(reasonLabel("sharper")).toBe("Skarpere");
  expect(reasonLabel("blurry", { pct: "82 %" })).toBe("Uskarpt (82 %)");
  expect(reasonLabel("finnes_ikke")).toBe("Forslag");
});
