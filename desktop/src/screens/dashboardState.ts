// Oversikt (dashboard): ren tilstand og avledede tall fra Dashboard-bridgen. Ingen effekter her (se dashboardFlow.ts).
// Kjernen sender koder og tall; teksten kommer fra i18n. Oversikten foreslår bare: ingenting her endrer eller sletter bilder.
import { KIND_CODES, type BridgeError, type KindCode } from "../bridge/types";
import type { Dashboard, FreedMonth, KindStat } from "../bridge/types-v1";

export type DashboardView =
  | { kind: "loading" }
  | { kind: "error"; error: BridgeError }
  | { kind: "empty"; data: Dashboard }
  | { kind: "ready"; data: Dashboard };

export type DashboardEvent =
  | { type: "loaded"; data: Dashboard }
  | { type: "failed"; error: BridgeError }
  | { type: "retry" };

export const initialDashboard: DashboardView = { kind: "loading" };

/** Samlet «kan ryddes»: summen av forslagstypene. Kjernen teller ikke beskyttede bilder under noen type. */
export function cleanable(d: Dashboard): { count: number; bytes: number } {
  return d.perKind.reduce((s, k) => ({ count: s.count + k.count, bytes: s.bytes + k.bytes }), { count: 0, bytes: 0 });
}

/** Tom oversikt: ingen forslag, ingenting i papirkurven, ingenting tømt, ingen avgjorte forslag. */
export function isEmpty(d: Dashboard): boolean {
  return cleanable(d).count === 0 && d.trashedCount === 0 && d.freedBytes === 0 && d.progress.total === 0;
}

export function reduceDashboard(s: DashboardView, e: DashboardEvent): DashboardView {
  switch (e.type) {
    case "loaded": return isEmpty(e.data) ? { kind: "empty", data: e.data } : { kind: "ready", data: e.data };
    // Feil under oppdatering skal ikke fjerne tall som allerede vises.
    case "failed": return s.kind === "ready" || s.kind === "empty" ? s : { kind: "error", error: e.error };
    case "retry": return s.kind === "error" ? initialDashboard : s;
  }
}

/** Én flis per forslagstype i fast rekkefølge (alle seks, også med 0), slik at tallene alltid stemmer med typene. */
export function kindTiles(d: Dashboard): KindStat[] {
  return KIND_CODES.map((kind) => d.perKind.find((k) => k.kind === kind) ?? { kind, count: 0, bytes: 0 });
}

/** 0..1 for fremdriftsringen. */
export function progressFraction(d: Dashboard): number {
  const p = d.progress.permille;
  return Number.isFinite(p) ? Math.min(1, Math.max(0, p / 1000)) : 0;
}

const isKind = (v: string | null): v is KindCode => v !== null && (KIND_CODES as readonly string[]).includes(v);

/** Hvor «Neste beste grep» fører. Ukjent forslagstype gir hele Gjennomgang. `null` = ingen knapp. */
export type NextTarget = { to: "review"; kind: KindCode | null } | { to: "trash" } | null;
export function nextTarget(d: Dashboard): NextTarget {
  switch (d.nextStep.kind) {
    case "review": return { to: "review", kind: isKind(d.nextStep.suggestionKind) ? d.nextStep.suggestionKind : null };
    case "empty_trash": return { to: "trash" };
    default: return null;
  }
}

/** Summer av «tømt per måned». */
export function freedTotals(months: FreedMonth[]): { count: number; bytes: number } {
  return months.reduce((s, m) => ({ count: s.count + m.count, bytes: s.bytes + m.bytes }), { count: 0, bytes: 0 });
}

/** `202611` -> Date (UTC, 1. i måneden) til Intl-formatering. */
export function monthDate(yearMonth: number): Date {
  return new Date(Date.UTC(Math.floor(yearMonth / 100), (yearMonth % 100) - 1, 1));
}
