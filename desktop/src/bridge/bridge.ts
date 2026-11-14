// Tynt lag over Tauri `invoke` (CoreBridge-embryo, ARKITEKTUR beslutning 11).
// Grensesnitt: én funksjon per Tauri-kommando i src-tauri/src/lib.rs, med samme navn i camelCase.
// Ingen forretningslogikk; feil kommer som `BridgeError` (koder, aldri tekst). UI-fasen (M6/M7) utvider og
// legger til MockBridge; UI-fundamentet finnes ikke ennå, så ingen skjermer bruker V1-funksjonene.
//
// Sikkerhet: ingen funksjon her sletter bilder. `protect(.., false)`, `deleteRule` og `mergePeople` kan angres
// (`protect(.., true)`, `restoreRule`, `unmergePerson`). `wipeFaceIndex` fjerner bare avledede ansiktsdata og
// må bekreftes av brukeren først. Beskyttede bilder gir `ack_required` til `decideCardsConfirmed` brukes.
import { invoke } from "@tauri-apps/api/core";
import type {
  AppState,
  CardPage,
  Condition,
  Dashboard,
  Decision,
  KindCode,
  Person,
  Protection,
  Rule,
  RuleDraft,
  RulePreview,
  Scope,
  SeriesCompare,
  Settings,
  Thresholds,
} from "./types";

export function getState(): Promise<AppState> {
  return invoke<AppState>("get_state");
}

export function startScan(full: boolean): Promise<void> {
  return invoke<void>("start_scan", { full });
}

export function listCards(
  kind: KindCode,
  limit: number,
  cursor: number | null = null,
): Promise<CardPage> {
  return invoke<CardPage>("list_cards", { kind, limit, cursor });
}

/** Returnerer batch-id (ett angre). */
export function decideCards(ids: number[], decision: Decision): Promise<number> {
  return invoke<number>("decide_cards", { ids, decision });
}

/** Etter `ack_required`: `confirmed` er konsekvenskodene brukeren har bekreftet. */
export function decideCardsConfirmed(
  ids: number[],
  decision: Decision,
  confirmed: string[],
): Promise<number> {
  return invoke<number>("decide_cards_confirmed", { ids, decision, confirmed });
}

export function undo(): Promise<number> {
  return invoke<number>("undo");
}

// ---- Beskyttelse ----

export function protect(scope: Scope, on: boolean): Promise<void> {
  return invoke<void>("protect", { scope, on });
}

export function listProtections(): Promise<Protection[]> {
  return invoke<Protection[]>("list_protections");
}

// ---- Regler ----

export function listRules(): Promise<Rule[]> {
  return invoke<Rule[]>("list_rules");
}

export function saveRule(draft: RuleDraft): Promise<number> {
  return invoke<number>("save_rule", { draft });
}

export function previewRule(conditions: Condition[]): Promise<RulePreview> {
  return invoke<RulePreview>("preview_rule", { conditions });
}

export function toggleRule(id: number, enabled: boolean): Promise<void> {
  return invoke<void>("toggle_rule", { id, enabled });
}

export function deleteRule(id: number): Promise<void> {
  return invoke<void>("delete_rule", { id });
}

export function restoreRule(id: number): Promise<void> {
  return invoke<void>("restore_rule", { id });
}

// ---- Personer og ansiktsindeks ----

export function listPeople(): Promise<Person[]> {
  return invoke<Person[]>("list_people");
}

export function namePerson(id: number, name: string | null): Promise<void> {
  return invoke<void>("name_person", { id, name });
}

/** Returnerer personen som ble stående. */
export function mergePeople(target: number, absorbed: number): Promise<number> {
  return invoke<number>("merge_people", { target, absorbed });
}

export function unmergePerson(absorbed: number): Promise<void> {
  return invoke<void>("unmerge_person", { absorbed });
}

export function protectPerson(id: number, protect: boolean): Promise<void> {
  return invoke<void>("protect_person", { id, protect });
}

export function getFaceIndex(): Promise<boolean> {
  return invoke<boolean>("get_face_index");
}

export function setFaceIndex(enabled: boolean): Promise<void> {
  return invoke<void>("set_face_index", { enabled });
}

/** Returnerer antall ansikter slettet. Bildene beholdes. Bekreft med brukeren først. */
export function wipeFaceIndex(): Promise<number> {
  return invoke<number>("wipe_face_index");
}

// ---- Oversikt, serier og innstillinger ----

export function getDashboard(): Promise<Dashboard> {
  return invoke<Dashboard>("get_dashboard");
}

export function compareSeries(series: number): Promise<SeriesCompare> {
  return invoke<SeriesCompare>("compare_series", { series });
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function setSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("set_settings", { settings });
}

export function getThresholds(): Promise<Thresholds> {
  return invoke<Thresholds>("get_thresholds");
}
