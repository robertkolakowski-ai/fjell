// Felles hjelpere for V1-Windows-spesene (`v1-win-*.spec.ts`): skjermliste, axe, tab-gjennomgang og layoutkontroller.
// Disse kjører mot MockBridge i Chromium (her og på windows-latest). De emulerer forced-colors, skalering og tastatur-bare bruk;
// de erstatter IKKE Narrator, NVDA eller ekte Windows-kontrasttemaer (se docs/v1/WINDOWS-V1.md for manuell protokoll).
import { createRequire } from "node:module";
import { expect, type Page } from "@playwright/test";
import { go, open, seedScan, type Lang } from "./helpers";

export interface V1Skjerm {
  /** Navn slik protokollen i docs/v1/WINDOWS-V1.md bruker det. */
  navn: string;
  hash: string;
  /** Forventet h1 (nb). */
  h1: string;
}

export const V1_SKJERMER: readonly V1Skjerm[] = [
  { navn: "Oversikt", hash: "#/", h1: "Hjem" },
  { navn: "Gjennomgang serier", hash: "#/review/series", h1: "Gjennomgang · Serier" },
  { navn: "Serie", hash: "#/series/41", h1: "Sammenlign bildene i serien" },
  { navn: "Serie med lukkede øyne", hash: "#/series/42", h1: "Sammenlign bildene i serien" },
  { navn: "Kortforklaring (serie)", hash: "#/cards/series", h1: "Rask opprydding" },
  { navn: "Kortforklaring (regel)", hash: "#/cards/rule", h1: "Rask opprydding" },
  { navn: "Personer (indeks av)", hash: "#/people", h1: "Personer" },
  { navn: "Regler", hash: "#/rules", h1: "Regler" },
  { navn: "Innstillinger (Sensitivitet, Ansikter, Beskyttelser)", hash: "#/settings", h1: "Innstillinger" },
];

const axeSrc = createRequire(import.meta.url).resolve("axe-core/axe.min.js");
export const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

/**
 * Kjører axe på siden og feiler med brudd (regel-id og de første elementene).
 * Under forced-colors (høykontrast) er `color-contrast` slått av i axe: axe leser designfargene (`--c-on-accent`) i stedet for
 * de tvungne systemfargene (observert: fgColor #06262b mot svart på en knapp hvis beregnede `color` er hvit). Kontrasten
 * kontrolleres der i stedet av `kontrastBeregnet` på de beregnede stilene.
 */
export async function axe(page: Page, where: string): Promise<void> {
  if (!(await page.evaluate(() => "axe" in window))) await page.addScriptTag({ path: axeSrc });
  await page.waitForTimeout(150);
  const forced = await page.evaluate(() => matchMedia("(forced-colors: active)").matches);
  const res = await page.evaluate(async ({ tags, forced }) => {
    // @ts-expect-error axe injiseres
    const r = await window.axe.run(document, { runOnly: { type: "tag", values: tags }, rules: forced ? { "color-contrast": { enabled: false } } : {} });
    return r.violations.map((v: { id: string; nodes: { target: unknown[]; failureSummary?: string }[] }) => ({
      id: v.id,
      nodes: v.nodes.slice(0, 3).map((n) => `${n.target.join(" ")} :: ${n.failureSummary?.split("\n")[1] ?? ""}`),
    }));
  }, { tags: AXE_TAGS, forced });
  expect(res, `axe-brudd: ${where}`).toEqual([]);
  if (forced) expect(await kontrastBeregnet(page), `kontrast (beregnede stiler, forced-colors): ${where}`).toEqual([]);
}

/** WCAG-kontrast (4.5:1) mellom beregnet tekstfarge og nærmeste ugjennomsiktige bakgrunn for all synlig tekst. Deaktiverte kontroller er unntatt. */
export async function kontrastBeregnet(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const parse = (c: string): [number, number, number, number] => {
      const m = c.match(/rgba?\(([^)]+)\)/);
      if (!m) return [0, 0, 0, 0];
      const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
      return [p[0], p[1], p[2], p[3] ?? 1];
    };
    const lum = ([r, g, b]: number[]) => {
      const f = (v: number) => { const x = v / 255; return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4; };
      return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
    };
    const bakgrunn = (el: Element): number[] => {
      for (let e: Element | null = el; e; e = e.parentElement) {
        const c = parse(getComputedStyle(e).backgroundColor);
        if (c[3] >= 0.99) return c;
      }
      return parse(getComputedStyle(document.documentElement).backgroundColor).slice(0, 3).concat(1);
    };
    const out: string[] = [];
    for (const el of Array.from(document.querySelectorAll<HTMLElement>("body *"))) {
      if (el.closest(".sr-only") || el instanceof SVGElement) continue;
      if (!el.checkVisibility({ checkVisibilityCSS: true })) continue;
      if (el.closest(":disabled, [aria-disabled=true]")) continue;
      const harTekst = Array.from(el.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? "").trim());
      if (!harTekst) continue;
      const fg = parse(getComputedStyle(el).color);
      const bg = bakgrunn(el);
      const [l1, l2] = [lum(fg), lum(bg)];
      const ratio = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
      if (ratio < 4.5) out.push(`${el.tagName.toLowerCase()}.${typeof el.className === "string" ? el.className.split(" ")[0] : ""} «${(el.textContent ?? "").trim().slice(0, 25)}» ${ratio.toFixed(2)}:1`);
    }
    return out.slice(0, 8);
  });
}

/** Åpner Hjem og kjører en rask skanning (mocken har da forslag i alle seks kategorier). */
export async function scanned(page: Page, hash = "#/", lang: Lang = "nb"): Promise<void> {
  await open(page, "#/", lang);
  if (lang === "nb") {
    await seedScan(page);
  } else {
    // Andre språk har andre ord: bruk strukturelle velgere (som i series.spec.ts).
    await go(page, "#/onboarding");
    await page.locator(".onb > .row .btn.primary").click();
    await page.locator(".onb > .row .btn.primary").click();
    await page.locator(".onb .btn.primary").filter({ hasText: /\d/ }).waitFor({ timeout: 15_000 });
  }
  await go(page, hash);
  await page.waitForTimeout(150);
}

export interface Stopp {
  tag: string;
  navn: string;
  /** Synlig fokusindikator: outline (overlever forced-colors) eller skygge/ramme (bare utenfor forced-colors). */
  outline: boolean;
  skygge: boolean;
  /** Elementets boks ligger helt innenfor vinduet bredde-messig. */
  innenforBredde: boolean;
  radioGruppe?: string;
}

/**
 * Flytter fokus til første fokuserbare element i dokumentet («Hopp til hovedinnhold»), som når et nytt vindu åpnes.
 * `blur()` og tekstmarkering flytter ikke Chromiums startpunkt for neste Tab (det blir stående på h1 etter rutebytte), så
 * første Tab trykkes først (gir tastaturmodus, slik at :focus-visible gjelder), og deretter settes fokus på lenken direkte.
 */
export async function tilToppen(page: Page): Promise<void> {
  await page.keyboard.press("Tab");
  await page.evaluate(() => {
    window.scrollTo(0, 0);
    document.querySelector<HTMLElement>("a[href]")?.focus();
  });
}

/** Trykker Tab (bare tastatur) til fokus står på et element hvis tilgjengelige navn (aria-label, label eller tekst) inneholder `del` (eller er lik, med `eksakt`). Feiler etter `maks` trinn. */
export async function tabTil(page: Page, del: string, opts: { eksakt?: boolean; maks?: number } = {}): Promise<void> {
  const maks = opts.maks ?? 80;
  for (let i = 0; i < maks; i++) {
    await page.keyboard.press("Tab");
    const treff = await page.evaluate(({ d, eksakt }) => {
      const a = document.activeElement as HTMLElement | null;
      if (!a || a === document.body) return false;
      const labels = a instanceof HTMLInputElement ? Array.from(a.labels ?? []).map((l) => l.textContent ?? "").join(" ") : "";
      const navn = (a.getAttribute("aria-label") || labels || a.textContent || "").replace(/\s+/g, " ").trim();
      return eksakt ? navn === d : navn.includes(d);
    }, { d: del, eksakt: !!opts.eksakt });
    if (treff) return;
  }
  throw new Error(`Tab nådde aldri «${del}» på ${maks} trinn`);
}

/** Trykker Tab fra toppen av siden til fokus forlater dokumentet (eller `maks` trinn). Alle tastetrykk, ingen mus. */
export async function tabGjennom(page: Page, maks = 120): Promise<Stopp[]> {
  await tilToppen(page);
  const stopp: Stopp[] = [];
  for (let i = 0; i < maks; i++) {
    if (i > 0) await page.keyboard.press("Tab");
    const s = await page.evaluate((): Stopp | null => {
      const a = document.activeElement as HTMLElement | null;
      if (!a || a === document.body || a === document.documentElement) return null;
      const cs = getComputedStyle(a);
      const r = a.getBoundingClientRect();
      const labels = a instanceof HTMLInputElement ? Array.from(a.labels ?? []).map((l) => l.textContent ?? "").join(" ") : "";
      const navn = (a.getAttribute("aria-label") || labels || a.textContent || (a as HTMLInputElement).value || a.getAttribute("placeholder") || "").trim().replace(/\s+/g, " ").slice(0, 50);
      const radio = a instanceof HTMLInputElement && a.type === "radio" ? a.name : undefined;
      return {
        tag: `${a.tagName.toLowerCase()}${a.getAttribute("role") ? `[${a.getAttribute("role")}]` : ""}`,
        navn: navn || a.getAttribute("aria-labelledby") || a.id || "(uten navn)",
        outline: cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) >= 2,
        skygge: cs.boxShadow !== "none",
        innenforBredde: r.left >= -1 && r.right <= innerWidth + 1,
        radioGruppe: radio,
      };
    });
    if (!s) break;
    stopp.push(s);
  }
  return stopp;
}

/** Alle synlige, fokuserbare elementer i dokumentet (utenfor lukkede dialoger), som beskrivelse. Radioknapper telles per gruppe. */
export async function fokuserbare(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const sel = 'a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
    const out = new Set<string>();
    for (const el of Array.from(document.querySelectorAll<HTMLElement>(sel))) {
      if (!el.checkVisibility({ checkVisibilityCSS: true })) continue;
      if (el.closest(".sr-only") && !(el instanceof HTMLAnchorElement)) continue;
      if (el instanceof HTMLInputElement && el.type === "radio") { out.add(`radio:${el.name}`); continue; }
      const labels = el instanceof HTMLInputElement ? Array.from(el.labels ?? []).map((l) => l.textContent ?? "").join(" ") : "";
      out.add((el.getAttribute("aria-label") || labels || el.textContent || (el as HTMLInputElement).value || el.getAttribute("placeholder") || "").trim().replace(/\s+/g, " ").slice(0, 50));
    }
    return Array.from(out);
  });
}

/** Finner avkutting og horisontal rulling (samme kriterier som herding.spec.ts, men bare det som trengs for V1-spesene). */
export async function layoutProblemer(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    const de = document.documentElement;
    if (de.scrollWidth > de.clientWidth + 1) out.push(`horisontal rulling: ${de.scrollWidth} > ${de.clientWidth}`);
    const desc = (el: Element) => `${el.tagName.toLowerCase()}${typeof el.className === "string" && el.className ? "." + el.className.trim().split(/\s+/)[0] : ""} «${(el.textContent ?? "").trim().slice(0, 30)}»`;
    for (const el of Array.from(document.querySelectorAll("body *"))) {
      if (el.closest(".sr-only") || el instanceof SVGElement) continue;
      const cs = getComputedStyle(el);
      if (cs.display === "none" || cs.visibility === "hidden") continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      const klipperX = cs.overflowX !== "visible" && cs.overflowX !== "auto" && cs.overflowX !== "scroll";
      const klipperY = cs.overflowY !== "visible" && cs.overflowY !== "auto" && cs.overflowY !== "scroll";
      if (cs.textOverflow !== "ellipsis") {
        if (klipperX && el.scrollWidth > el.clientWidth + 1) out.push(`avkuttet bredde: ${desc(el)} (${el.scrollWidth} > ${el.clientWidth})`);
        if (klipperY && el.scrollHeight > el.clientHeight + 1) out.push(`avkuttet høyde: ${desc(el)} (${el.scrollHeight} > ${el.clientHeight})`);
      }
      if (el.matches("button, a, [role=button]") && el.scrollWidth > el.clientWidth + 1) out.push(`kontroll renner over: ${desc(el)}`);
      if (!el.closest("dialog") && (r.right > innerWidth + 1 || r.left < -1) && !el.closest("[style*='overflow']")) out.push(`utenfor vinduet: ${desc(el)} (${Math.round(r.left)}..${Math.round(r.right)} av ${innerWidth})`);
    }
    return out.slice(0, 8);
  });
}
