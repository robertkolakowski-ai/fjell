// Forklaring av et begrunnelsesmerke: tekst per ReasonCode og tallene fra `Detail` (prosent, antall, serieposisjon, regel, person).
// Ren funksjon uten I/O. Teksten kommer fra i18n (`card.explain.*`); kjernen sender bare kode og tall. Forklaringen sier hvorfor
// appen foreslår noe, aldri hva brukeren bør gjøre.
import type { Detail, ReasonCode } from "../bridge/types";
import { fmt, type TFunction } from "../lib/i18n";
import type { RuleNames } from "./reason";

/** Én rad i tallene under forklaringen («Plass i serien: 2 av 4»). */
export interface ExplainFact {
  /** Stabil kode til tester og stil; teksten er `label`/`value`. */
  code: "blur" | "percent" | "count" | "position" | "rule" | "person";
  label: string;
  value: string;
}

export interface ExplainInput {
  reason: ReasonCode;
  detail: Detail;
  /** Brukes når begrunnelsen er uskarphet og `detail` ikke har prosent. */
  confidence?: number;
  ruleNames?: RuleNames;
}

/** Prosent for uskarphet: `detail.percent` hvis kjernen sender det, ellers kortets `confidence`. */
function blurPercent(i: ExplainInput): number | null {
  if (i.detail.kind === "percent") return i.detail.value;
  return i.confidence ?? null;
}

/** Forklaringsteksten for én begrunnelse. */
export function explainBody(i: ExplainInput, t: TFunction): string {
  const d = i.detail;
  switch (i.reason) {
    case "ExactCopy": return t("card.explain.exact_copy");
    case "VisualDuplicate": return t("card.explain.visual_duplicate");
    case "Blurry": return t("card.explain.blurry", { pct: fmt.percent(blurPercent(i) ?? 0) });
    case "Screenshot": return t("card.explain.screenshot");
    case "HigherResolution": return t("card.explain.higher_resolution");
    case "LessCompressed": return t("card.explain.less_compressed");
    case "Sharper": return t("card.explain.sharper");
    case "Older": return t("card.explain.older");
    case "ShotPlatformFlag": return t("card.explain.shot_platform_flag");
    case "ShotScreenSize": return t("card.explain.shot_screen_size");
    case "ShotNoCameraExif": return t("card.explain.shot_no_camera_exif");
    case "ShotPng": return t("card.explain.shot_png");
    case "SeriesMember":
      return t("card.explain.series_member", { index: d.kind === "position" ? d.index : 0, of: d.kind === "position" ? d.of : 0 });
    case "KeeperBestOverall": return t("card.explain.keeper_best_overall");
    case "KeeperEyesOpen": return t("card.explain.keeper_eyes_open");
    case "ClosedEyes": return t("card.explain.closed_eyes", { count: d.kind === "count" ? d.value : 1 });
    case "Rule": return t("card.explain.rule", { name: ruleName(i, t) });
    case "ProtectedPerson": return t("card.explain.protected_person");
  }
}

function ruleName(i: ExplainInput, t: TFunction): string {
  if (i.detail.kind !== "rule") return t("rules.name.unnamed");
  return i.ruleNames?.get(i.detail.id) ?? t("rules.name.unnamed");
}

/** Tallene bak merket, i fast rekkefølge. Tom liste når det ikke finnes noen tall. */
export function explainFacts(i: ExplainInput, t: TFunction): ExplainFact[] {
  const d = i.detail;
  switch (d.kind) {
    case "percent":
      return [{ code: i.reason === "Blurry" ? "blur" : "percent", label: t(i.reason === "Blurry" ? "card.explain.fact.blur" : "card.explain.fact.percent"), value: fmt.percent(d.value) }];
    case "count":
      return [{ code: "count", label: t("card.explain.fact.count"), value: fmt.number(d.value) }];
    case "position":
      return [{ code: "position", label: t("card.explain.fact.position"), value: t("card.explain.value.position", { index: d.index, of: d.of }) }];
    case "rule":
      return [{ code: "rule", label: t("card.explain.fact.rule"), value: ruleName(i, t) }];
    case "person":
      return [{ code: "person", label: t("card.explain.fact.person"), value: t("card.explain.value.person") }];
    case "none":
      if (i.reason === "Blurry" && i.confidence !== undefined) {
        return [{ code: "blur", label: t("card.explain.fact.blur"), value: fmt.percent(i.confidence) }];
      }
      return [];
  }
}
