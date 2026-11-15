// Regler: rene hjelpere (ingen I/O). Vilkårsbygger (rader <-> Condition), lesbar setning og feilkode -> tekstnøkkel.
// Regler foreslår bare. Ingenting her flytter, sletter eller godtar noe.
import type { BridgeError } from "../bridge/types";
import type { Condition } from "../bridge/types-v1";
import type { MessageKey, TFunction } from "../lib/i18n";

export type ConditionKind = Condition["kind"];

export const CONDITION_KINDS: readonly ConditionKind[] = [
  "is_screenshot", "is_blurry", "is_exact_duplicate", "older_than_days", "larger_than_bytes", "smaller_than_bytes", "in_album", "in_folder", "mime",
];

/** Maks lengde på regelnavn (som kjernen). */
export const RULE_NAME_MAX = 80;

export type SizeUnit = "kb" | "mb" | "gb";
export const SIZE_UNITS: readonly SizeUnit[] = ["kb", "mb", "gb"];
const UNIT_BYTES: Record<SizeUnit, number> = { kb: 1_000, mb: 1_000_000, gb: 1_000_000_000 };

/** En rad i vilkårsbyggeren. `text` er det brukeren har skrevet; tolkes først når regelen forhåndsvises eller lagres. */
export interface ConditionRow {
  key: number;
  kind: ConditionKind;
  text: string;
  unit: SizeUnit;
}

export const kindHasValue = (k: ConditionKind): boolean =>
  k === "older_than_days" || k === "larger_than_bytes" || k === "smaller_than_bytes" || k === "in_album" || k === "in_folder" || k === "mime";
export const kindIsNumeric = (k: ConditionKind): boolean => kindHasValue(k) && k !== "in_folder" && k !== "mime";

let nextKey = 1;
export function newRow(kind: ConditionKind = "is_screenshot"): ConditionRow {
  return { key: nextKey++, kind, text: "", unit: "mb" };
}

/** Største enhet som gir et helt tall (20 000 000 byte -> 20 MB). */
export function bestUnit(bytes: number): { value: number; unit: SizeUnit } {
  for (const u of ["gb", "mb", "kb"] as const) {
    if (bytes >= UNIT_BYTES[u] && bytes % UNIT_BYTES[u] === 0) return { value: bytes / UNIT_BYTES[u], unit: u };
  }
  return { value: bytes / UNIT_BYTES.kb, unit: "kb" };
}

export function rowFromCondition(c: Condition): ConditionRow {
  const r = newRow(c.kind);
  switch (c.kind) {
    case "older_than_days": case "in_album": r.text = String(c.value); break;
    case "larger_than_bytes": case "smaller_than_bytes": {
      const b = bestUnit(c.value);
      r.text = String(b.value);
      r.unit = b.unit;
      break;
    }
    case "in_folder": case "mime": r.text = c.value; break;
    default: break;
  }
  return r;
}

/** Tall fra tekstfelt (komma eller punktum). Tomt eller ugyldig gir 0, som bridgen avviser med `invalid_condition`. */
function parseNumber(s: string): number {
  const n = Number(s.trim().replace(",", "."));
  return Number.isFinite(n) ? n : 0;
}

/** Raden som vilkår. Ugyldig innhold sendes med uendret (verdi 0 eller tom tekst) så bridgen svarer med feilkode. */
export function conditionOf(r: ConditionRow): Condition {
  switch (r.kind) {
    case "is_screenshot": case "is_blurry": case "is_exact_duplicate": return { kind: r.kind };
    case "older_than_days": return { kind: r.kind, value: parseNumber(r.text) };
    case "in_album": return { kind: r.kind, value: parseNumber(r.text) };
    case "larger_than_bytes": case "smaller_than_bytes": return { kind: r.kind, value: Math.round(parseNumber(r.text) * UNIT_BYTES[r.unit]) };
    case "in_folder": case "mime": return { kind: r.kind, value: r.text.trim() };
  }
}

export const conditionsOf = (rows: readonly ConditionRow[]): Condition[] => rows.map(conditionOf);

/**
 * Feil skjermen selv fanger før noe sendes til bridgen. Kjernen avviser bare tom regel og tomme tekstvilkår, og tar imot tall
 * som 0 («eldre enn 0 dager» treffer alt), og forhåndsvisning validerer ingenting. Derfor krever skjermen mer enn kjernen:
 * minst ett vilkår, tekst som ikke er tom og tall som er hele og over 0 (V1-PLAN åpent spørsmål 4). Samme koder som kjernen.
 */
export function conditionsError(rows: readonly ConditionRow[]): BridgeError | null {
  if (rows.length === 0) return { code: "empty_rule", detail: "" };
  for (const c of conditionsOf(rows)) {
    if (!("value" in c)) continue;
    const bad = typeof c.value === "string" ? c.value === "" : !Number.isInteger(c.value) || c.value < 1;
    if (bad) return { code: "invalid_condition", detail: c.kind };
  }
  return null;
}

/** Stabil nøkkel for «har vilkårene endret seg siden forhåndsvisningen». */
export const conditionsKey = (rows: readonly ConditionRow[]): string => JSON.stringify(conditionsOf(rows));

/** Ett vilkår som tekst fra i18n. `bytes` formaterer størrelser i gjeldende språk. */
export function describeCondition(c: Condition, t: TFunction, bytes: (n: number) => string): string {
  switch (c.kind) {
    case "is_screenshot": return t("rules.cond.screenshot");
    case "is_blurry": return t("rules.cond.blurry");
    case "is_exact_duplicate": return t("rules.cond.exact_duplicate");
    case "older_than_days": return t("rules.cond.older_than", { days: c.value });
    case "larger_than_bytes": return t("rules.cond.larger_than", { size: bytes(c.value) });
    case "smaller_than_bytes": return t("rules.cond.smaller_than", { size: bytes(c.value) });
    case "in_album": return t("rules.cond.in_album", { name: String(c.value) });
    case "in_folder": return t("rules.cond.in_folder", { name: c.value });
    case "mime": return t("rules.cond.mime", { type: c.value });
  }
}

/** Hele regelen som én lesbar setning. */
export function describeRule(conditions: readonly Condition[], t: TFunction, bytes: (n: number) => string): string {
  return t("rules.sentence", { conditions: conditions.map((c) => describeCondition(c, t, bytes)).join("; ") });
}

export const kindLabelKey = (k: ConditionKind): MessageKey => `rules.kind.${k}` as MessageKey;

const ERROR_KEYS: Readonly<Record<string, MessageKey>> = {
  empty_rule: "error.empty_rule",
  empty_name: "error.empty_name",
  name_too_long: "error.name_too_long",
  invalid_condition: "error.invalid_condition",
  rule_not_found: "error.rule_not_found",
  unknown_target: "error.unknown_target",
  empty_folder: "error.empty_folder",
};

/** Feilkode fra bridgen -> nøkkel for teksten som vises. Ukjent kode gir generell feil. */
export function errorKeyOf(e: BridgeError | null | undefined): MessageKey {
  return (e && ERROR_KEYS[e.code]) || "rules.error.generic";
}
