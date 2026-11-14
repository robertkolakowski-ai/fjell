// V1-kommandoene: tynt lag over Tauri `invoke`, én funksjon per V1-kommando i src-tauri/src/lib.rs med samme navn i
// camelCase. Ingen forretningslogikk; feil kommer som koder (`ErrorDto`), aldri tekst. Ingen skjermer bruker disse
// ennå, og de er derfor ikke del av `CoreBridge`/`MockBridge` (bridge.ts); når V1-skjermene kommer, flyttes de dit.
// Beslutning med bekreftelse (`decideCards` med `confirmed`), `setKeeper` og skallets innstillinger ligger i CoreBridge.
//
// Sikkerhet: ingen funksjon her sletter bilder. `protect(.., false)`, `deleteRule` og `mergePeople` kan angres
// (`protect(.., true)`, `restoreRule`, `unmergePerson`). `wipeFaceIndex` fjerner bare avledede ansiktsdata og
// må bekreftes av brukeren først.
import { invoke } from "@tauri-apps/api/core";
import type {
  Condition,
  Dashboard,
  Person,
  Protection,
  Rule,
  RuleDraft,
  RulePreview,
  Scope,
  SeriesCompare,
  SuggestionSettings,
  Thresholds,
} from "./types-v1";

// ---- Beskyttelse ----

export function protect(scope: Scope, on: boolean): Promise<void> {
  return invoke<void>("protect", { scope, on });
}

export function listProtections(): Promise<Protection[]> {
  return invoke<Protection[]>("list_protections");
}

// ---- Forslagsregler ----

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

export function getSuggestionSettings(): Promise<SuggestionSettings> {
  return invoke<SuggestionSettings>("get_suggestion_settings");
}

export function setSuggestionSettings(settings: SuggestionSettings): Promise<SuggestionSettings> {
  return invoke<SuggestionSettings>("set_suggestion_settings", { settings });
}

export function getThresholds(): Promise<Thresholds> {
  return invoke<Thresholds>("get_thresholds");
}
