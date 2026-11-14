// V1-data og rene hjelpefunksjoner til MockBridge: personer, regler, album, stier, terskler og dashboard-tall.
// Deterministisk (ingen tilfeldighet, ingen klokke utenom det kalleren gir), slik at skjermtester og e2e er stabile.
// Semantikken følger V1-PLAN del 7: feilkoder er stabile koder, regler foreslår bare, beskyttede bilder får aldri regeltreff.
import type { BridgeError } from "./types";
import type { Condition, Scope, SensitivityCode, Thresholds } from "./types-v1";

/** Antall bilder mocken lar «finnes i indeksen» (`unknown_target` utenfor 1..MOCK_ASSETS). */
export const MOCK_ASSETS = 1200;
/** Samlet størrelse for biblioteket (dashboard). */
export const MOCK_BYTES = 4_800_000_000;
/** Bit i `protectFlags` som mocken setter på bilder brukeren selv har beskyttet (bilde, mappe, album eller person). Reservert hos kjernen for flagg fra plattformen (1, 2, 4, 8). */
export const SCOPE_FLAG = 16;
export const RULE_NAME_MAX = 80;
export const PERSON_NAME_MAX = 100;

const err = (code: string, detail = ""): BridgeError => ({ code, detail });

// ---- Personer ----

export interface MockPerson {
  id: number;
  name: string | null;
  /** Egne ansikter (uten sammenslåtte). */
  faces: number;
  assets: number[];
  /** Personen er slått inn i denne (id), eller null. */
  mergedInto: number | null;
}

/** Ansiktsindeksen er av som standard; disse vises først når den er på. */
export function seedPeople(): MockPerson[] {
  return [
    { id: 1, name: "Kari", faces: 14, assets: [1, 2, 3, 4, 7, 8], mergedInto: null },
    { id: 2, name: null, faces: 6, assets: [5, 6, 9], mergedInto: null },
    { id: 3, name: null, faces: 4, assets: [10, 11], mergedInto: null },
    { id: 4, name: "Jonas", faces: 9, assets: [12, 13, 14, 15, 25], mergedInto: null },
  ];
}

// ---- Regler ----

export interface MockRule {
  id: number;
  name: string;
  conditions: Condition[];
  enabled: boolean;
  /** Mykt slettet (kan hentes tilbake). */
  deleted: boolean;
}

/** To regler som matcher kortene `seedCards()` lager (kort 31-33 og 34-36). */
export function seedRules(): MockRule[] {
  return [
    { id: 1, name: "Gamle skjermbilder", conditions: [{ kind: "is_screenshot" }, { kind: "older_than_days", value: 90 }], enabled: true, deleted: false },
    { id: 2, name: "Store filer", conditions: [{ kind: "larger_than_bytes", value: 20_000_000 }], enabled: true, deleted: false },
  ];
}

const CONDITION_KINDS: readonly string[] = [
  "is_screenshot", "is_blurry", "is_exact_duplicate", "older_than_days", "larger_than_bytes", "smaller_than_bytes", "in_album", "in_folder", "mime",
];

/** Som `suggest_rules::validate`: tom regel gir `empty_rule`; vilkår uten innhold gir `invalid_condition`. */
export function validateConditions(conditions: readonly Condition[]): void {
  if (conditions.length === 0) throw err("empty_rule");
  for (const c of conditions) {
    if (!CONDITION_KINDS.includes(c.kind)) throw err("invalid_condition", c.kind);
    if ("value" in c) {
      const v = c.value;
      const bad = typeof v === "string" ? v.trim() === "" : !Number.isFinite(v) || v <= 0;
      if (bad) throw err("invalid_condition", c.kind);
    }
  }
}

/** Første fritekst i indeksen: brukerens eget regelnavn (maks 80 tegn, ikke tomt). */
export function cleanRuleName(name: string): string {
  const n = name.trim();
  if (n === "") throw err("empty_name");
  if ([...n].length > RULE_NAME_MAX) throw err("name_too_long");
  return n;
}

/** Bildene som regelen «treffer» (id 201..212). Deterministisk av vilkårene; 1 til 8 treff. */
export function ruleCandidates(conditions: readonly Condition[]): number[] {
  let h = 2166136261;
  for (const ch of JSON.stringify(conditions)) h = Math.imul(h ^ ch.charCodeAt(0), 16777619) >>> 0;
  const n = 1 + (h % 8);
  const start = (h >>> 8) % 12;
  return Array.from({ length: n }, (_, i) => 201 + ((start + i) % 12));
}

/** Størrelse på et regeltreff (deterministisk). */
export const ruleHitBytes = (asset: number): number => 2_000_000 + (asset % 12) * 100_000;

// ---- Album og stier ----

/** Album i mocken (id -> bilder). 1 = skjermbilder, 2 = ferie. */
export const MOCK_ALBUMS: Readonly<Record<number, readonly number[]>> = {
  1: [19, 20, 21, 22, 23, 24],
  2: [1, 2, 3, 4, 5, 6],
};

const FOLDERS = ["Ferie", "Familie", "Skjermbilder"] as const;

/** Mappe-stien et bilde ligger i (til mappebeskyttelse). */
export function assetPath(library: string, asset: number): string {
  const sep = library.includes("\\") ? "\\" : "/";
  const base = library.replace(/[\\/]+$/, "");
  return `${base}${sep}${FOLDERS[(Math.abs(asset) - 1 + FOLDERS.length * 1000) % FOLDERS.length]}${sep}IMG_${String(asset).padStart(4, "0")}.jpg`;
}

const normPath = (p: string): string => p.trim().replace(/\\/g, "/").toLowerCase();

/** Kanonisk beskyttelse: mappesti trimmes og mister avsluttende skråstrek; tom mappesti gir `empty_folder` (ville truffet alt). */
export function normalizeScope(s: Scope): Scope {
  if (s.scope !== "asset" && s.scope !== "folder" && s.scope !== "album" && s.scope !== "person") throw err("bad_request", "scope");
  if (typeof s.target !== "string") throw err("bad_request", "target");
  if (s.scope === "folder") {
    const t = s.target.trim().replace(/[\\/]+$/, "");
    if (t === "") throw err("empty_folder");
    return { scope: "folder", target: t };
  }
  return { scope: s.scope, target: s.target.trim() };
}

export const scopeKey = (s: Scope): string => `${s.scope}:${s.scope === "folder" ? normPath(s.target) : s.target}`;

/** Folder-prefiks er rent prefiks på stien (V1-PLAN åpent spørsmål 18). */
export function folderMatches(library: string, asset: number, prefix: string): boolean {
  return normPath(assetPath(library, asset)).startsWith(normPath(prefix));
}

// ---- Terskler (V1-PLAN del 7: sensitivitet flytter startverdiene) ----

export function thresholdsFor(s: Record<"visual_dup" | "blurry" | "series", SensitivityCode>): Thresholds {
  const pick = <T>(code: SensitivityCode, low: T, normal: T, high: T): T => (code === "low" ? low : code === "high" ? high : normal);
  return {
    visualMaxHamming: pick(s.visual_dup, 4, 6, 8),
    blurMaxScore: Math.round(0.006 * pick(s.blurry, 0.7, 1, 1.3) * 1e6) / 1e6,
    seriesGapSecs: pick(s.series, 5, 10, 20),
    closedEyesMinConfidence: pick(s.series, 0.9, 0.8, 0.7),
  };
}

// ---- Dashboard ----

/** Dashboard-tall per måned (eldst først), de siste seks månedene frem til `now` (ms). Summerer til MOCK_ASSETS. */
export function monthSeries(now: number): { yearMonth: number; count: number; bytes: number }[] {
  const counts = [150, 210, 180, 260, 220, 180];
  const d = new Date(now);
  return counts.map((count, i) => {
    const m = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() - (counts.length - 1 - i), 1));
    return { yearMonth: m.getUTCFullYear() * 100 + m.getUTCMonth() + 1, count, bytes: Math.round((MOCK_BYTES * count) / MOCK_ASSETS) };
  });
}

export const yearMonthOf = (ms: number): number => {
  const d = new Date(ms);
  return d.getUTCFullYear() * 100 + d.getUTCMonth() + 1;
};

export const TOP_FOLDERS = (library: string): { path: string; count: number; bytes: number }[] => {
  const sep = library.includes("\\") ? "\\" : "/";
  const base = library.replace(/[\\/]+$/, "");
  return [
    { path: `${base}${sep}Familie`, count: 508, bytes: 1_750_000_000 },
    { path: `${base}${sep}Ferie`, count: 412, bytes: 1_850_000_000 },
    { path: `${base}${sep}Skjermbilder`, count: 280, bytes: 1_200_000_000 },
  ];
};
