// WCAG 2.x kontrasttest for alle fargepar i src/lib/tokens.css. Feiler (exit 1) under AA.
// Kjør: npm run contrast  (også `just contrast`). Ingen avhengigheter.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const TOKENS = fileURLToPath(new URL("../src/lib/tokens.css", import.meta.url));

/** Par som må bestå. kind: "text" = 4.5:1 (1.4.3), "ui" = 3:1 (1.4.11 ikoner, kanter, fokus, ring). */
export const PAIRS = [
  ["text", "--c-text", "--c-bg"],
  ["text", "--c-text", "--c-surface"],
  ["text", "--c-text", "--c-surface-sunken"],
  ["text", "--c-text-muted", "--c-bg"],
  ["text", "--c-text-muted", "--c-surface"],
  ["text", "--c-text-muted", "--c-surface-sunken"],
  ["text", "--c-on-accent", "--c-accent"],
  ["text", "--c-on-accent", "--c-accent-hover"],
  ["text", "--c-accent", "--c-surface"],
  ["text", "--c-accent", "--c-bg"],
  ["text", "--c-badge-text", "--c-badge-bg"],
  ["text", "--c-badge-warn-text", "--c-badge-warn-bg"],
  ["text", "--c-danger", "--c-surface"],
  ["text", "--c-danger", "--c-bg"],
  ["text", "--c-inverse-text", "--c-inverse-bg"],
  ["text", "--c-inverse-action", "--c-inverse-bg"],
  ["ui", "--c-focus", "--c-bg"],
  ["ui", "--c-focus", "--c-surface"],
  ["ui", "--c-border-strong", "--c-bg"],
  ["ui", "--c-border-strong", "--c-surface"],
  ["ui", "--c-selected", "--c-surface"],
  ["ui", "--c-selected", "--c-bg"],
  ["ui", "--c-ring-track", "--c-surface"],
  ["ui", "--c-ring-track", "--c-bg"],
  ["ui", "--c-ring-fill", "--c-surface"],
  ["ui", "--c-ring-fill", "--c-bg"],
  ["ui", "--c-inverse-action", "--c-inverse-bg"], // fokusring i Toast
  // Loft (M10): sidemeny, hover, semantiske flater
  ["text", "--c-text", "--c-nav-bg"],
  ["text", "--c-text-muted", "--c-nav-bg"],
  ["text", "--c-text", "--c-nav-active-bg"],
  ["text", "--c-text-muted", "--c-nav-active-bg"],
  ["text", "--c-text", "--c-surface-hover"],
  ["text", "--c-text-muted", "--c-surface-hover"],
  ["text", "--c-text", "--c-accent-subtle"],
  ["text", "--c-text-muted", "--c-accent-subtle"],
  ["text", "--c-accent", "--c-accent-subtle"],
  ["text", "--c-text", "--c-info-bg"],
  ["text", "--c-ok", "--c-surface"],
  ["text", "--c-ok", "--c-bg"],
  ["text", "--c-ok", "--c-ok-bg"],
  ["text", "--c-text", "--c-ok-bg"],
  ["text", "--c-danger", "--c-danger-bg"],
  ["text", "--c-text", "--c-danger-bg"],
  ["ui", "--c-focus", "--c-nav-bg"],
  ["ui", "--c-focus", "--c-nav-active-bg"],
  ["ui", "--c-focus", "--c-surface-hover"],
  ["ui", "--c-focus", "--c-accent-subtle"],
  ["ui", "--c-focus", "--c-surface-sunken"],
  ["ui", "--c-border-strong", "--c-nav-bg"],
  ["ui", "--c-border-strong", "--c-surface-hover"],
  ["ui", "--c-border-strong", "--c-accent-subtle"],
  ["ui", "--c-selected", "--c-accent-subtle"],
  ["ui", "--c-selected", "--c-surface-hover"],
  ["ui", "--c-ok", "--c-surface"],
  ["ui", "--c-danger", "--c-surface"],
];

const MIN = { text: 4.5, ui: 3 };

export function parseHex(v) {
  const m = /^#([0-9a-f]{6})$/i.exec(v.trim());
  if (!m) throw new Error(`Ugyldig heksfarge: «${v}»`);
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function luminance([r, g, b]) {
  const f = (c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

export function ratio(fg, bg) {
  const [a, b] = [luminance(parseHex(fg)), luminance(parseHex(bg))].sort((x, y) => y - x);
  return (a + 0.05) / (b + 0.05);
}

/** Leser tema-blokker merket `/* theme:<navn> *\/`. Flere blokker med samme navn må være identiske. */
export function parseThemes(css) {
  const parts = css.split(/\/\*\s*theme:([a-z]+)[^*]*\*\//);
  const themes = {};
  for (let i = 1; i < parts.length; i += 2) {
    const name = parts[i];
    if (name === "forced") continue;
    const decls = {};
    for (const m of parts[i + 1].matchAll(/(--c-[a-z-]+)\s*:\s*([^;]+);/g)) decls[m[1]] = m[2].trim();
    if (themes[name]) {
      const prev = themes[name];
      const keys = new Set([...Object.keys(prev), ...Object.keys(decls)]);
      for (const k of keys) {
        if (prev[k] !== decls[k]) throw new Error(`Tema «${name}»: blokkene er ulike for ${k} (${prev[k]} mot ${decls[k]})`);
      }
    } else themes[name] = decls;
  }
  return themes;
}

export function check(css) {
  const themes = parseThemes(css);
  const rows = [];
  const fails = [];
  for (const [name, t] of Object.entries(themes)) {
    for (const [kind, fg, bg] of PAIRS) {
      if (!(fg in t) || !(bg in t)) {
        fails.push(`${name}: mangler token ${!(fg in t) ? fg : bg}`);
        continue;
      }
      const r = ratio(t[fg], t[bg]);
      const ok = r >= MIN[kind];
      rows.push({ name, kind, fg, bg, r, ok });
      if (!ok) fails.push(`${name}: ${fg} på ${bg} = ${r.toFixed(2)}:1 (krever ${MIN[kind]}:1, ${kind})`);
    }
  }
  return { themes: Object.keys(themes), rows, fails };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { themes, rows, fails } = check(readFileSync(TOKENS, "utf8"));
  if (process.argv.includes("--verbose")) for (const r of rows) console.log(`${r.ok ? "ok  " : "FEIL"} ${r.name.padEnd(6)} ${r.fg} / ${r.bg} ${r.r.toFixed(2)}`);
  if (fails.length) {
    console.error(`contrast: ${fails.length} brudd under WCAG AA`);
    for (const f of fails) console.error("  " + f);
    process.exit(1);
  }
  console.log(`contrast: ${rows.length} par i ${themes.length} temaer (${themes.join(", ")}) består WCAG AA`);
}
