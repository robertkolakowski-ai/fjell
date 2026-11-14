// UX-6 herding: høykontrast (forced-colors), redusert bevegelse og Windows-skalering 100/150/200 % uten avkutting.
// Alle tester går gjennom samme omvisning (e2e/tour.ts). Skjermbilder lagres med SHOTS=1 i docs/screens/m8/.
import { expect, test, type Page } from "@playwright/test";
import { shot, tour } from "./tour";

/** Finner avkutting og horisontal rulling i gjeldende DOM. Tom liste = OK. */
async function layoutProblems(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    const de = document.documentElement;
    if (de.scrollWidth > de.clientWidth + 1) out.push(`horisontal rulling: ${de.scrollWidth} > ${de.clientWidth}`);
    const desc = (el: Element) => `${el.tagName.toLowerCase()}${el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\s+/)[0] : ""} «${(el.textContent ?? "").trim().slice(0, 30)}»`;
    let last = 0;
    for (const el of Array.from(document.querySelectorAll("body *"))) {
      if (el.closest(".sr-only") || el instanceof SVGElement) continue;
      const cs = getComputedStyle(el);
      if (cs.display === "none" || cs.visibility === "hidden") continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      const clipsX = cs.overflowX !== "visible" && cs.overflowX !== "auto" && cs.overflowX !== "scroll";
      const clipsY = cs.overflowY !== "visible" && cs.overflowY !== "auto" && cs.overflowY !== "scroll";
      // Bevisst avkorting med ellipsis (Badge) er unntatt: full tekst ligger i DOM og title.
      if (cs.textOverflow !== "ellipsis") {
        if (clipsX && el.scrollWidth > el.clientWidth + 1) out.push(`avkuttet bredde: ${desc(el)} (${el.scrollWidth} > ${el.clientWidth})`);
        if (clipsY && el.scrollHeight > el.clientHeight + 1) out.push(`avkuttet høyde: ${desc(el)} (${el.scrollHeight} > ${el.clientHeight})`);
      }
      // Knapper og lenker: tekst må få plass i boksen.
      if (el.matches("button, a, [role=button]") && el.scrollWidth > el.clientWidth + 1) out.push(`kontroll renner over: ${desc(el)}`);
      // Synlige elementer utenfor vinduet i bredden (utenfor rullbar forelder).
      if (!el.closest("dialog") && (r.right > innerWidth + 1 || r.left < -1) && !el.closest("[style*='overflow']")) out.push(`utenfor vinduet: ${desc(el)} (${Math.round(r.left)}..${Math.round(r.right)} av ${innerWidth})`);
      last++;
    }
    if (last === 0) out.push("ingen synlige elementer (tom side?)");
    return out.slice(0, 8);
  });
}

/** Innhold nederst må ikke skjules av den faste bunnlinjen (bunnlinjen kan vokse ved lang tekst og stor skrift). */
async function bottomBarCoversContent(page: Page): Promise<string | null> {
  return page.evaluate(() => {
    const bar = document.querySelector<HTMLElement>(".bar[role=region], .bar");
    if (!bar || getComputedStyle(bar).position !== "fixed") return null;
    window.scrollTo(0, document.documentElement.scrollHeight);
    const barTop = bar.getBoundingClientRect().top;
    let maxBottom = 0;
    for (const el of Array.from(document.querySelectorAll("main *"))) {
      if (el.closest(".sr-only") || bar.contains(el) || el.classList.contains("spacer") || el.contains(bar) || el.querySelector(":scope > .spacer")) continue;
      const r = el.getBoundingClientRect();
      if (r.width && r.height) maxBottom = Math.max(maxBottom, r.bottom);
    }
    return maxBottom > barTop + 1 ? `bunnlinjen dekker innhold: innhold slutter ${Math.round(maxBottom)}, bunnlinjen starter ${Math.round(barTop)}` : null;
  });
}

const SCALES = [
  // Windows-skalering emulert: CSS-viewport = fysisk oppløsning / skalering. text = «Gjør tekst større» (rot-skriftstørrelse).
  { name: "100pct", w: 1920, h: 1080, dpr: 1, text: 100 },
  { name: "150pct", w: 1280, h: 720, dpr: 1.5, text: 150 },
  { name: "200pct", w: 960, h: 540, dpr: 2, text: 200 },
  { name: "200pct-minvindu", w: 480, h: 520, dpr: 2, text: 200 }, // Tauri minWidth/minHeight, verste tilfelle
] as const;

for (const s of SCALES) {
  test(`skalering ${s.name}: ingen avkutting og ingen horisontal rulling (nb)`, async ({ browser }) => {
    test.setTimeout(180_000);
    const ctx = await browser.newContext({ viewport: { width: s.w, height: s.h }, deviceScaleFactor: s.dpr });
    const page = await ctx.newPage();
    await page.addInitScript((pct) => {
      document.addEventListener("DOMContentLoaded", () => { document.documentElement.style.fontSize = `${pct}%`; });
    }, s.text);
    const problems: string[] = [];
    await tour(page, async (name) => {
      expect(await page.evaluate(() => devicePixelRatio)).toBe(s.dpr);
      const rootPx = await page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).fontSize));
      expect(rootPx).toBeCloseTo(16 * (s.text / 100), 1);
      for (const p of await layoutProblems(page)) problems.push(`${name}: ${p}`);
      const cover = await bottomBarCoversContent(page);
      if (cover) problems.push(`${name}: ${cover}`);
      if (s.name.startsWith("200")) await shot(page, `${name}-skalering-${s.name}`); // 100/150 % gir ingen nye funn; bilder bare for 200 %
    });
    expect(problems, problems.join("\n")).toEqual([]);
    await ctx.close();
  });
}

test("skalering 200 %: pseudo-lokale (+40 % tekst) uten avkutting på minstevindu", async ({ browser }) => {
  test.setTimeout(180_000);
  const ctx = await browser.newContext({ viewport: { width: 480, height: 520 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await page.addInitScript(() => { document.addEventListener("DOMContentLoaded", () => { document.documentElement.style.fontSize = "200%"; }); });
  const problems: string[] = [];
  await tour(page, async (name) => {
    for (const p of await layoutProblems(page)) problems.push(`${name}: ${p}`);
    const cover = await bottomBarCoversContent(page);
    if (cover) problems.push(`${name}: ${cover}`);
  }, "pseudo");
  expect(problems, problems.join("\n")).toEqual([]);
  await ctx.close();
});

// ---------- Høykontrast (Windows kontrasttemaer = forced-colors: active) ----------
for (const scheme of ["light", "dark"] as const) {
  test(`høykontrast (forced-colors) ${scheme}: kantlinjer, fokus og tilstand uten farge`, async ({ browser }) => {
    test.setTimeout(180_000);
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, forcedColors: "active", colorScheme: scheme, reducedMotion: "reduce" });
    const page = await ctx.newPage();
    const problems: string[] = [];
    await tour(page, async (name) => {
      expect(await page.evaluate(() => matchMedia("(forced-colors: active)").matches)).toBe(true);
      const res = await page.evaluate(() => {
        const out: string[] = [];
        // Knapper skal ha synlig kant (forced-colors fjerner bakgrunn; kanten er det som skiller dem).
        for (const b of Array.from(document.querySelectorAll<HTMLElement>("main button, header button, dialog button"))) {
          const r = b.getBoundingClientRect();
          if (!r.width || b.closest(".sr-only")) continue;
          const cs = getComputedStyle(b);
          if (cs.borderTopStyle === "none" || parseFloat(cs.borderTopWidth) < 1) out.push(`knapp uten kant: «${(b.textContent ?? "").trim().slice(0, 30)}»`);
        }
        // Valgt flis: tilstand må ha annet enn farge (hake-ikon eller tykkere kant).
        for (const t of Array.from(document.querySelectorAll<HTMLElement>(".tile[aria-pressed=true]"))) {
          const hasIcon = !!t.querySelector("svg");
          const w = parseFloat(getComputedStyle(t).borderTopWidth) + parseFloat(getComputedStyle(t.querySelector(".img") ?? t).borderTopWidth);
          if (!hasIcon && w < 3) out.push("valgt flis er bare markert med farge");
        }
        // Systemfarger: tekst skal ikke ha eget forgrunn/bakgrunn-par som bryter forced-colors (forced-color-adjust: none på tekst).
        for (const el of Array.from(document.querySelectorAll<HTMLElement>("main *"))) {
          if (getComputedStyle(el).forcedColorAdjust === "none" && el.textContent?.trim() && !(el instanceof SVGElement)) out.push(`forced-color-adjust: none på tekst: ${el.tagName.toLowerCase()}.${el.className}`);
        }
        return out;
      });
      problems.push(...res.map((r) => `${name}: ${r}`));
      // Fokusring må være synlig (outline overlever forced-colors).
      await page.keyboard.press("Tab");
      const focus = await page.evaluate(() => {
        const a = document.activeElement as HTMLElement | null;
        if (!a || a === document.body) return null;
        const cs = getComputedStyle(a);
        return { style: cs.outlineStyle, width: parseFloat(cs.outlineWidth) };
      });
      if (focus && (focus.style === "none" || focus.width < 2)) problems.push(`${name}: fokusring usynlig (${focus.style} ${focus.width}px)`);
      for (const p of await layoutProblems(page)) problems.push(`${name}: ${p}`);
      await shot(page, `${name}-hoykontrast-${scheme}`);
    });
    expect(problems, problems.join("\n")).toEqual([]);
    await ctx.close();
  });
}

// ---------- prefers-reduced-motion ----------
test("redusert bevegelse: ingen animasjoner eller overganger over 1 ms på noen skjerm", async ({ browser }) => {
  test.setTimeout(180_000);
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, reducedMotion: "reduce" });
  const page = await ctx.newPage();
  const problems: string[] = [];
  await tour(page, async (name) => {
    const res = await page.evaluate(() => {
      const out: string[] = [];
      const secs = (v: string) => v.split(",").map((x) => (x.trim().endsWith("ms") ? parseFloat(x) / 1000 : parseFloat(x)));
      for (const el of Array.from(document.querySelectorAll("*"))) {
        for (const pseudo of [null, "::before", "::after"]) {
          const cs = getComputedStyle(el, pseudo);
          const d = Math.max(...secs(cs.transitionDuration), ...secs(cs.animationDuration));
          if (d > 0.001) out.push(`${el.tagName.toLowerCase()}${pseudo ?? ""}.${el.className} varighet ${d}s`);
          if (cs.scrollBehavior === "smooth") out.push(`${el.tagName.toLowerCase()} scroll-behavior: smooth`);
        }
      }
      const running = document.getAnimations().filter((a) => Number(a.effect?.getTiming().duration ?? 0) > 1);
      if (running.length) out.push(`${running.length} kjørende animasjoner (WAAPI/CSS)`);
      return out.slice(0, 5);
    });
    problems.push(...res.map((r) => `${name}: ${r}`));
  });
  expect(problems, problems.join("\n")).toEqual([]);
  await ctx.close();
});

test("redusert bevegelse: kontroll, uten preferansen finnes overganger (testen er ikke tom)", async ({ browser }) => {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, reducedMotion: "no-preference" });
  const page = await ctx.newPage();
  await page.goto("/?mockDelay=0#/");
  await page.locator("h1").first().waitFor();
  const longest = await page.evaluate(() => {
    const secs = (v: string) => v.split(",").map((x) => (x.trim().endsWith("ms") ? parseFloat(x) / 1000 : parseFloat(x)));
    return Math.max(...Array.from(document.querySelectorAll("*")).flatMap((el) => [...secs(getComputedStyle(el).transitionDuration), ...secs(getComputedStyle(el).animationDuration)]));
  });
  expect(longest).toBeGreaterThan(0.05);
  await ctx.close();
});
