import { describe, expect, it } from "vitest";
import { CARD_KEYS } from "./cardsState";
import { EVERYWHERE, SCREEN_KEYS, SCREEN_LABEL, helpScreenOf, isHelpKey, type HelpScreen } from "./helpKeys";
import { setLanguage, t } from "../lib/i18n";
import { HOME, parseHash, type Route } from "../router";

const SCREENS = Object.keys(SCREEN_KEYS) as HelpScreen[];
const ROUTES: Route[] = [
  { name: "onboarding" }, HOME, { name: "review", kind: null }, { name: "group", id: 1 },
  { name: "cards", kind: null }, { name: "trash" }, { name: "settings" },
];

describe("F1-hjelp", () => {
  it("alle skjermer har tabell og navn, og hver rute mapper til en skjerm", () => {
    for (const r of ROUTES) {
      const s = helpScreenOf(r);
      expect(SCREENS).toContain(s);
      expect(SCREEN_LABEL[s]).toBeTruthy();
    }
    expect(new Set(ROUTES.map(helpScreenOf)).size).toBe(SCREENS.length);
  });
  it("F1 finnes i «Overalt» (gjelder alle skjermer)", () => {
    expect(EVERYWHERE.map((r) => r.keys)).toContain("F1");
  });
  it("skjermspesifikke taster: Gjennomgang, Gruppe, Papirkurv og Kortmodus har egne; Hjem, Innstillinger og Onboarding har ingen", () => {
    expect(SCREEN_KEYS.review.map((r) => r.keys)).toEqual(["Space", "Ctrl+A", "Delete"]);
    expect(SCREEN_KEYS.group.map((r) => r.keys)).toEqual(["1–9"]);
    expect(SCREEN_KEYS.trash.map((r) => r.keys)).toEqual(["Space"]);
    expect(SCREEN_KEYS.home).toEqual([]);
    expect(SCREEN_KEYS.settings).toEqual([]);
    expect(SCREEN_KEYS.onboarding).toEqual([]);
  });
  it("regler 6.4: ingen Alt/Win/Ctrl+W/Tab/F4, ingen enkelttegn-bokstaver", () => {
    const all = [...EVERYWHERE, ...Object.values(SCREEN_KEYS).flat()];
    for (const r of all) {
      expect(r.keys).not.toMatch(/Alt|Win|Meta|Ctrl\+(W|Tab|F4)/);
      expect(r.keys).not.toMatch(/^[A-Za-z]$/);
    }
  });
  it("kortmodus-tabellen speiler CARD_KEYS (én sannhet for tastene)", () => {
    const cards = SCREEN_KEYS.cards.map((r) => r.keys).join(" ");
    expect(cards).toContain("Ctrl+Z");
    expect(cards).toContain("↑ / Delete");
    expect(CARD_KEYS.map((k) => k.label)).toEqual(expect.arrayContaining(["←", "↑ / Delete", "→", "Ctrl+Z"]));
  });
  it("alle tekster finnes på nb og en (ingen manglende oversettelse)", () => {
    const keys = [...EVERYWHERE, ...Object.values(SCREEN_KEYS).flat()].map((r) => r.action);
    const labels = Object.values(SCREEN_LABEL);
    for (const lang of ["nb", "en"] as const) {
      setLanguage(lang);
      for (const k of [...keys, ...labels]) {
        const text = (t as unknown as (key: string) => string)(k);
        expect(text, `${lang} ${k}`).not.toBe("");
        expect(text, `${lang} ${k}`).not.toBe(k);
      }
    }
    setLanguage("nb");
  });
  it("F1 uten modifikatorer åpner hjelp, men ikke på kortmodus (egen dialog)", () => {
    expect(isHelpKey({ key: "F1" }, HOME)).toBe(true);
    expect(isHelpKey({ key: "F1" }, parseHash("#/review"))).toBe(true);
    expect(isHelpKey({ key: "F1" }, parseHash("#/cards"))).toBe(false);
    expect(isHelpKey({ key: "F1", ctrlKey: true }, HOME)).toBe(false);
    expect(isHelpKey({ key: "F2" }, HOME)).toBe(false);
  });
});
