// Papirkurv («Nylig slettet», PLAN-UX-I18N 3.6 og 4): ViewState + ren reduce. Ingen I/O.
// Tøm: appens ark med valg Windows Papirkurv (standard) eller permanent. Permanent har eget trinn med avkrysning
// (PermanentAck: checked + riktig antall). Filer Papirkurv ikke kan ta (`Unsupported`: volum uten Papirkurv, lang sti,
// UNC) blir liggende urørt i karantenen. Brukeren velger «La ligge» (standard) eller «Slett permanent» (egen bekreftelse).
import type { AppState, BridgeError, EmptyResult, PermanentAck, TrashedItem } from "../bridge/types";

export type PurgeMode = "recycle" | "permanent";
export type TrashPhase = "loading" | "empty" | "list" | "confirmPurge" | "confirmUnsupported" | "confirmPermanent" | "purging" | "purged" | "error";

export interface TrashInfo {
  count: number;
  bytes: number;
  expired: number;
  freedBytes: number;
  freedSince: number | null;
  days: number;
}

export interface PurgeOutcome {
  mode: PurgeMode;
  count: number;
  /** Elementer som ikke kunne flyttes og ligger fortsatt i «Nylig slettet». */
  marked: number;
  /** Elementer Papirkurv ikke støtter: ligger urørt i karantenemappen (ikke slettet). */
  kept: number;
  freedBytes: number;
}

export interface TrashState {
  phase: TrashPhase;
  info: TrashInfo | null;
  /** Bilder i «Nylig slettet», eldste først (per-bilde-listen). */
  items: TrashedItem[];
  /** Valgte bilder (asset-id) til «Gjenopprett». */
  selected: ReadonlySet<number>;
  /** Gjenoppretting pågår. */
  restoring: boolean;
  /** Antall gjenopprettet i siste handling (til melding). null = ingen melding. */
  restored: number | null;
  /** Gjenoppretting feilet: ingenting er endret. */
  restoreFailed: boolean;
  mode: PurgeMode;
  ack: boolean;
  /** Antall filer Papirkurv ikke kunne ta i siste forsøk (`Unsupported`). De ligger urørt i karantenen. */
  unsupported: number;
  /** Permanent-trinnet ble åpnet fra «kan ikke flyttes»-arket: Avbryt går tilbake dit. */
  fromUnsupported: boolean;
  outcome: PurgeOutcome | null;
  error: BridgeError | null;
}

export type TrashEvent =
  | { type: "loaded"; state: AppState; items?: TrashedItem[] }
  | { type: "toggle"; asset: number }
  | { type: "selectAll" }
  | { type: "clearSelection" }
  | { type: "restoring" }
  | { type: "restoredOk"; count: number }
  | { type: "restoreFailed" }
  | { type: "failed"; error: BridgeError }
  | { type: "openPurge" }
  | { type: "setMode"; mode: PurgeMode }
  | { type: "continue" }
  | { type: "setAck"; value: boolean }
  | { type: "cancel" }
  /** «La ligge i karantenen» (standard i `Unsupported`-arket). Ingenting endres. */
  | { type: "keepUnsupported" }
  /** «Slett permanent …» i `Unsupported`-arket: går til permanent-trinnet med avkrysning. */
  | { type: "deleteUnsupported" }
  /** Slett permanent trykket. Krever avkrysning. */
  | { type: "confirmPermanent" }
  | { type: "purged"; mode: PurgeMode; result: EmptyResult }
  | { type: "purgeFailed"; error: BridgeError }
  | { type: "dismiss" }
  | { type: "reload" };

export const initialTrash: TrashState = {
  phase: "loading", info: null, items: [], selected: new Set(), restoring: false, restored: null, restoreFailed: false,
  mode: "recycle", ack: false, unsupported: 0, fromUnsupported: false, outcome: null, error: null,
};

export type TrashSectionId = "expired" | "soon" | "other";
export interface TrashSection {
  id: TrashSectionId;
  items: TrashedItem[];
}
const DAY = 86_400;
/** «Utløper snart» = under eller lik 7 dager igjen (PLAN-UX-I18N 3.6). */
export const SOON_DAYS = 7;

/** Gruppert liste: Utløpt, Utløper snart (≤ 7 dager), øvrige. Tomme grupper utelates. `nowS` = sekunder siden epoch. */
export function sectionsOf(items: readonly TrashedItem[], nowS: number): TrashSection[] {
  const pick = (id: TrashSectionId, f: (i: TrashedItem) => boolean): TrashSection => ({ id, items: items.filter(f) });
  const soon = (i: TrashedItem) => !i.expired && i.expiresAt - nowS <= SOON_DAYS * DAY;
  return [
    pick("expired", (i) => i.expired),
    pick("soon", soon),
    pick("other", (i) => !i.expired && !soon(i)),
  ].filter((s) => s.items.length > 0);
}

/** Dager igjen til utløp, rundet opp og aldri under 0. */
export const daysLeft = (i: TrashedItem, nowS: number): number => Math.max(0, Math.ceil((i.expiresAt - nowS) / DAY));

export const infoOf = (s: AppState): TrashInfo => ({
  count: s.trashCount, bytes: s.trashBytes, expired: s.trashExpired, freedBytes: s.freedBytes,
  freedSince: s.freedSince ?? null, days: s.trashDays,
});

/** Parametere til bridge.emptyTrash for valgt modus. Permanent sender alltid avkrysning og antall. */
export const ackFor = (s: TrashState): PermanentAck | null =>
  s.mode === "permanent" && s.info ? { checked: s.ack, count: s.info.count } : null;

/** «La ligge»: lukk arket og vis resultatet (det som ble flyttet, og at resten ligger urørt). */
const keepUnsupported = (s: TrashState): TrashState => {
  const o = s.outcome ?? { mode: "recycle" as const, count: 0, marked: 0, kept: 0, freedBytes: 0 };
  return { ...s, phase: "purged", ack: false, fromUnsupported: false, outcome: { ...o, kept: s.unsupported }, unsupported: 0 };
};

const listOf = (info: TrashInfo): TrashPhase => (info.count === 0 ? "empty" : "list");

export function reduceTrash(s: TrashState, e: TrashEvent): TrashState {
  switch (e.type) {
    case "reload":
      return { ...initialTrash };
    case "loaded": {
      const info = infoOf(e.state);
      const items = e.items ?? s.items;
      const live = new Set(items.map((i) => i.asset));
      const selected = new Set([...s.selected].filter((a) => live.has(a)));
      // Under og rett etter tømming beholdes ark/resultat; ny tilstand fra kjernen oppdaterer bare tallene.
      if (s.phase === "purging" || s.phase === "purged" || s.phase === "confirmPurge" || s.phase === "confirmUnsupported" || s.phase === "confirmPermanent") return { ...s, info, items, selected };
      return { ...s, phase: listOf(info), info, items, selected, error: null };
    }
    case "failed":
      return { ...s, phase: "error", error: e.error };
    case "toggle": {
      if (s.phase !== "list" || s.restoring || !s.items.some((i) => i.asset === e.asset)) return s;
      const next = new Set(s.selected);
      if (!next.delete(e.asset)) next.add(e.asset);
      return { ...s, selected: next, restored: null, restoreFailed: false };
    }
    case "selectAll":
      return s.phase === "list" && !s.restoring ? { ...s, selected: new Set(s.items.map((i) => i.asset)), restored: null, restoreFailed: false } : s;
    case "clearSelection":
      return s.phase === "list" && !s.restoring ? { ...s, selected: new Set() } : s;
    case "restoring":
      // Ingenting valgt eller allerede i gang: ingen handling.
      return s.phase === "list" && !s.restoring ? { ...s, restoring: true, restored: null, restoreFailed: false } : s;
    case "restoredOk":
      return s.restoring ? { ...s, restoring: false, restored: e.count, selected: new Set(), restoreFailed: false } : s;
    case "restoreFailed":
      return s.restoring ? { ...s, restoring: false, restoreFailed: true } : s;
    case "openPurge":
      if (s.phase !== "list" || !s.info) return s;
      return { ...s, phase: "confirmPurge", mode: "recycle", ack: false, unsupported: 0, fromUnsupported: false, error: null };
    case "setMode":
      return s.phase === "confirmPurge" ? { ...s, mode: e.mode } : s;
    case "continue":
      if (s.phase !== "confirmPurge") return s;
      return s.mode === "permanent" ? { ...s, phase: "confirmPermanent", ack: false } : { ...s, phase: "purging" };
    case "setAck":
      return s.phase === "confirmPermanent" ? { ...s, ack: e.value } : s;
    case "confirmPermanent":
      return s.phase === "confirmPermanent" && s.ack ? { ...s, phase: "purging" } : s;
    case "cancel":
      // Esc eller Avbryt i `Unsupported`-arket er det samme som «La ligge». Fra permanent-trinnet tilbake til valgene.
      if (s.phase === "confirmUnsupported") return keepUnsupported(s);
      if (s.phase === "confirmPermanent" && s.fromUnsupported) return { ...s, phase: "confirmUnsupported", mode: "recycle", ack: false };
      return s.phase === "confirmPurge" || s.phase === "confirmPermanent" ? { ...s, phase: "list", ack: false } : s;
    case "keepUnsupported":
      return s.phase === "confirmUnsupported" ? keepUnsupported(s) : s;
    case "deleteUnsupported":
      return s.phase === "confirmUnsupported" ? { ...s, phase: "confirmPermanent", mode: "permanent", ack: false, fromUnsupported: true } : s;
    case "purged": {
      const r = e.result;
      const outcome: PurgeOutcome = {
        mode: e.mode, count: r.purged, marked: Math.max(0, r.keptTrashed - r.unsupported), kept: 0, freedBytes: r.freedBytes,
      };
      // Papirkurv-modus med filer som ikke støttes: de ligger urørt i karantenen. Brukeren velger, aldri stille sletting.
      if (e.mode === "recycle" && r.unsupported > 0) {
        return { ...s, phase: "confirmUnsupported", unsupported: r.unsupported, fromUnsupported: false, mode: "recycle", ack: false, outcome };
      }
      return { ...s, phase: "purged", ack: false, fromUnsupported: false, unsupported: 0, outcome };
    }
    case "purgeFailed":
      // Ingenting er slettet: tilbake til listen med feilmelding.
      return { ...s, phase: "list", ack: false, error: e.error };
    case "dismiss":
      return s.phase === "purged" ? { ...s, phase: s.info && s.info.count > 0 ? "list" : "empty", outcome: null } : s;
  }
}

/** Utløpsmerking: antall elementer som er utløpt (aldri stille slettet). */
export const hasExpired = (i: TrashInfo | null): boolean => !!i && i.expired > 0;
