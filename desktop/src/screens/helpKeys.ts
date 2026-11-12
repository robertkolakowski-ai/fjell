// F1-hjelp: hurtigtaster per skjerm (PLAN-UX-I18N 6.4). Ren data + rene funksjoner. Tabellen viser bare taster som virker.
// Kortmodus har egen F1-dialog (CARD_KEYS i cardsState.ts er tabellen der); App åpner ikke denne hjelpen på kortmodus.
import type { MessageKey } from "../lib/i18n";
import type { Route } from "../router";

export type HelpScreen = "onboarding" | "home" | "review" | "group" | "cards" | "trash" | "settings";

export interface HelpRow {
  /** Tastenavn slik de vises (Ctrl+A, Delete, 1–9). Mellomrom oversettes i visningen. */
  keys: string;
  action: MessageKey;
}

/** Gjelder alle skjermer. */
export const EVERYWHERE: readonly HelpRow[] = [
  { keys: "F1", action: "help.action.help" },
  { keys: "Esc", action: "help.action.close" },
  { keys: "Tab", action: "help.action.tab" },
  { keys: "Enter", action: "help.action.activate" },
];

/** Spesifikke taster per skjerm. Tom liste = ingen egne taster (bare «Overalt»). */
export const SCREEN_KEYS: Readonly<Record<HelpScreen, readonly HelpRow[]>> = {
  onboarding: [],
  home: [],
  // Piler, Delete og Mellomrom er bare aktive når listen har fokus, aldri i tekstfelt (6.4).
  review: [
    { keys: "Space", action: "help.action.toggle" },
    { keys: "Ctrl+A", action: "review.select.all" },
    { keys: "Delete", action: "help.action.trash_selected" },
  ],
  group: [{ keys: "1–9", action: "help.action.best" }],
  // Kortmodus viser sin egen dialog; disse er speilet fra CARD_KEYS for fullstendighet.
  cards: [
    { keys: "←", action: "cards.keep" },
    { keys: "↑ / Delete", action: "cards.trash" },
    { keys: "→", action: "cards.skip" },
    { keys: "Space", action: "common.zoom" },
    { keys: "Ctrl+Z", action: "common.undo" },
  ],
  trash: [{ keys: "Space", action: "help.action.toggle" }],
  settings: [],
};

export function helpScreenOf(r: Route): HelpScreen {
  return r.name;
}

/** Skjermnavn til undertittelen i F1-dialogen. */
export const SCREEN_LABEL: Readonly<Record<HelpScreen, MessageKey>> = {
  onboarding: "help.screen.onboarding",
  home: "nav.home",
  review: "nav.review",
  group: "help.screen.group",
  cards: "help.screen.cards",
  trash: "nav.trash",
  settings: "nav.settings",
};

/** F1 uten modifikatorer åpner/lukker hjelpen. Kortmodus har egen dialog og er unntatt. */
export function isHelpKey(e: { key: string; ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean }, r: Route): boolean {
  return e.key === "F1" && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && r.name !== "cards";
}
