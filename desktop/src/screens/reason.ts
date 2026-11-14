// Begrunnelsesmerke for et kort: kjernen sender kode og tall (`Card.detail`), teksten kommer fra i18n (`reason.*`).
import type { CoreBridge } from "../bridge/core-bridge";
import type { Card } from "../bridge/types";
import { fmt, reasonLabel, t } from "../lib/i18n";
import { reasonSnake } from "./reviewState";

/** Regelnavn per regel-id. Navnene er brukerens egne og ligger bare i regellisten, ikke på kortet. */
export type RuleNames = ReadonlyMap<number, string>;

/** Henter regelnavnene til «Regel: …»-merket. Feil gir tomt kart: merket viser da «uten navn» i stedet for å hindre skjermen. */
export async function loadRuleNames(b: CoreBridge): Promise<RuleNames> {
  try {
    return new Map((await b.listRules()).map((r) => [r.id, r.name] as const));
  } catch {
    return new Map();
  }
}

/** Ferdig begrunnelsesmerke: prosent (uskarpt), serieplass («Bilde 2 av 4») og regelnavn følger kortets `detail`. */
export function cardReason(c: Card, names: RuleNames = new Map()): string {
  const d = c.detail ?? { kind: "none" };
  return reasonLabel(reasonSnake(c.reason), {
    pct: fmt.percent(c.confidence),
    index: d.kind === "position" ? d.index : undefined,
    of: d.kind === "position" ? d.of : undefined,
    name: d.kind === "rule" ? (names.get(d.id) ?? t("rules.name.unnamed")) : undefined,
  });
}
