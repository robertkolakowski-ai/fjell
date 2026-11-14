// ProtectToggle: tilgjengelighet og tilstand i markup (server-rendering; interaksjon og axe dekkes av e2e/protect-toggle.spec.ts).
import { describe, expect, it } from "vitest";
import { render } from "svelte/server";
import { setLanguage } from "../i18n";
import ProtectToggle from "./ProtectToggle.svelte";

const html = (on: boolean, extra: Record<string, unknown> = {}): string =>
  render(ProtectToggle, { props: { scope: { scope: "asset", target: "7" }, protected: on, onchange: () => undefined, label: "Beskytt dette bildet", ...extra } }).body;

describe("ProtectToggle", () => {
  setLanguage("nb");
  it("aria-pressed følger tilstanden og er en ekte knapp", () => {
    expect(html(false)).toMatch(/<button[^>]*aria-pressed="false"/);
    expect(html(true)).toMatch(/<button[^>]*aria-pressed="true"/);
    expect(html(true)).toMatch(/<button[^>]*type="button"/);
  });
  it("navnet (synlig tekst) endres ikke med tilstanden; tilstandsordet er bare visuelt", () => {
    for (const on of [false, true]) {
      const h = html(on);
      expect(h).toContain("Beskytt dette bildet");
      expect(h).toMatch(/<span class="state[^"]*" aria-hidden="true">(På|Av)<\/span>/);
    }
    expect(html(true)).toContain(">På<");
    expect(html(false)).toContain(">Av<");
  });
  it("ikon og tekst, ikke bare farge: hake når på, skjold når av", () => {
    expect(html(true)).toContain("M5 12.5l4.5 4.5L19 7.5");
    expect(html(false)).toContain("M12 3.5 19 6v5.5");
    expect(html(true)).toMatch(/class="icon svelte-\w+ filled"/);
    expect(html(false)).not.toMatch(/ filled"/);
  });
  it("skjermleserbeskrivelse kobles med aria-describedby og nevner omfanget og kravet om bekreftelse", () => {
    const h = html(false, { scope: { scope: "folder", target: "C:\\Bilder" } });
    const id = /aria-describedby="([^"]+)"/.exec(h)?.[1];
    expect(id).toBeTruthy();
    expect(h).toContain(`id="${id}"`);
    expect(h).toMatch(/Mappe: beskyttede bilder krever alltid din egen bekreftelse/);
    expect(h).toContain('data-scope="folder"');
  });
  it("sperret gir disabled", () => {
    expect(html(false, { disabled: true })).toMatch(/<button[^>]*disabled/);
    expect(html(false)).not.toMatch(/<button[^>]*disabled/);
  });
  it("engelsk tekst", () => {
    setLanguage("en");
    try {
      const h = html(false, { label: "Protect this photo" });
      expect(h).toContain("Protect this photo");
      expect(h).toContain("protected photos always need your own confirmation");
      expect(h).toMatch(/>Off</);
    } finally { setLanguage("nb"); }
  });
});
