import { test, expect } from "vitest";
import { readFileSync } from "node:fs";
import { check, parseThemes, ratio } from "./contrast.mjs";

const css = readFileSync(new URL("../src/lib/tokens.css", import.meta.url), "utf8");

test("kjente verdier (WCAG)", () => {
  expect(ratio("#000000", "#ffffff").toFixed(2)).toBe("21.00");
  expect(ratio("#777777", "#ffffff").toFixed(2)).toBe("4.48");
});

test("tokens.css består AA i lys, mørk og høykontrast", () => {
  const r = check(css);
  expect(r.themes.sort()).toEqual(["dark", "hc", "light"]);
  expect(r.fails).toEqual([]);
});

test("testen feiler under AA", () => {
  const bad = css.replace("--c-text-muted: #444d59;", "--c-text-muted: #aaaaaa;");
  expect(check(bad).fails.some((f) => f.startsWith("light: --c-text-muted"))).toBe(true);
});

test("duplikate mørk-blokker må være like", () => {
  const bad = css.replace("--c-bg: #101419;", "--c-bg: #111419;");
  expect(() => parseThemes(bad)).toThrow(/ulike/);
});
