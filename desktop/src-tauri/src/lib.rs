//! Tauri-limet for Fjell desktop: registrerer kommandoer som delegerer til `fjell-shell`.
//!
//! Avhenger av: `tauri`, `fjell-shell`.
//! Grensesnitt: `run()` (kalles fra `main.rs`). Kommandoer: `get_state`, `start_scan`,
//! `list_cards`, `decide_cards`, `undo`, og V1: `decide_cards_confirmed`, `protect`, `list_protections`,
//! `list_rules`, `save_rule`, `preview_rule`, `toggle_rule`, `delete_rule`, `restore_rule`, `list_people`,
//! `name_person`, `merge_people`, `unmerge_person`, `protect_person`, `get_face_index`, `set_face_index`,
//! `wipe_face_index`, `get_dashboard`, `compare_series`, `get_settings`, `set_settings`, `get_thresholds`.
//! Ingen logikk her; ingen sletting (beslutning 17). Samme kontrakt som `desktop/src/bridge/bridge.ts`.

use fjell_shell::dto::{CardPageDto, DecisionDto, ErrorDto, StateDto};
use fjell_shell::dto_v1::{
    ConditionDto, DashboardDto, PersonDto, ProtectionDto, RuleDraftDto, RuleDto, RulePreviewDto,
    ScopeDto, SeriesCompareDto, SettingsDto, ThresholdsDto,
};
use fjell_shell::Shell;
use tauri::State;

#[tauri::command]
fn get_state(shell: State<'_, Shell>) -> StateDto {
    shell.state()
}

#[tauri::command]
fn start_scan(shell: State<'_, Shell>, full: bool) -> Result<(), ErrorDto> {
    shell.start_scan(full)
}

#[tauri::command]
fn list_cards(
    shell: State<'_, Shell>,
    kind: String,
    limit: usize,
    cursor: Option<u64>,
) -> Result<CardPageDto, ErrorDto> {
    shell.cards(&kind, limit, cursor)
}

#[tauri::command]
fn decide_cards(
    shell: State<'_, Shell>,
    ids: Vec<u64>,
    decision: DecisionDto,
) -> Result<u64, ErrorDto> {
    shell.decide(&ids, decision)
}

#[tauri::command]
fn undo(shell: State<'_, Shell>) -> Result<u64, ErrorDto> {
    shell.undo()
}

#[tauri::command]
fn decide_cards_confirmed(
    shell: State<'_, Shell>,
    ids: Vec<u64>,
    decision: DecisionDto,
    confirmed: Vec<String>,
) -> Result<u64, ErrorDto> {
    shell.decide_confirmed(&ids, decision, &confirmed)
}

#[tauri::command]
fn protect(shell: State<'_, Shell>, scope: ScopeDto, on: bool) -> Result<(), ErrorDto> {
    shell.protect(&scope, on)
}

#[tauri::command]
fn list_protections(shell: State<'_, Shell>) -> Result<Vec<ProtectionDto>, ErrorDto> {
    shell.protections()
}

#[tauri::command]
fn list_rules(shell: State<'_, Shell>) -> Result<Vec<RuleDto>, ErrorDto> {
    shell.rules()
}

#[tauri::command]
fn save_rule(shell: State<'_, Shell>, draft: RuleDraftDto) -> Result<u64, ErrorDto> {
    shell.save_rule(&draft)
}

#[tauri::command]
fn preview_rule(
    shell: State<'_, Shell>,
    conditions: Vec<ConditionDto>,
) -> Result<RulePreviewDto, ErrorDto> {
    shell.preview_rule(&conditions)
}

#[tauri::command]
fn toggle_rule(shell: State<'_, Shell>, id: u64, enabled: bool) -> Result<(), ErrorDto> {
    shell.toggle_rule(id, enabled)
}

#[tauri::command]
fn delete_rule(shell: State<'_, Shell>, id: u64) -> Result<(), ErrorDto> {
    shell.delete_rule(id)
}

#[tauri::command]
fn restore_rule(shell: State<'_, Shell>, id: u64) -> Result<(), ErrorDto> {
    shell.restore_rule(id)
}

#[tauri::command]
fn list_people(shell: State<'_, Shell>) -> Result<Vec<PersonDto>, ErrorDto> {
    shell.people()
}

#[tauri::command]
fn name_person(shell: State<'_, Shell>, id: u64, name: Option<String>) -> Result<(), ErrorDto> {
    shell.name_person(id, name)
}

#[tauri::command]
fn merge_people(shell: State<'_, Shell>, target: u64, absorbed: u64) -> Result<u64, ErrorDto> {
    shell.merge_people(target, absorbed)
}

#[tauri::command]
fn unmerge_person(shell: State<'_, Shell>, absorbed: u64) -> Result<(), ErrorDto> {
    shell.unmerge_person(absorbed)
}

#[tauri::command]
fn protect_person(shell: State<'_, Shell>, id: u64, protect: bool) -> Result<(), ErrorDto> {
    shell.protect_person(id, protect)
}

#[tauri::command]
fn get_face_index(shell: State<'_, Shell>) -> bool {
    shell.face_index()
}

#[tauri::command]
fn set_face_index(shell: State<'_, Shell>, enabled: bool) -> Result<(), ErrorDto> {
    shell.set_face_index(enabled)
}

#[tauri::command]
fn wipe_face_index(shell: State<'_, Shell>) -> Result<u64, ErrorDto> {
    shell.wipe_face_index()
}

#[tauri::command]
fn get_dashboard(shell: State<'_, Shell>) -> Result<DashboardDto, ErrorDto> {
    shell.dashboard()
}

#[tauri::command]
fn compare_series(shell: State<'_, Shell>, series: u64) -> Result<SeriesCompareDto, ErrorDto> {
    shell.compare_series(series)
}

#[tauri::command]
fn get_settings(shell: State<'_, Shell>) -> SettingsDto {
    shell.settings()
}

#[tauri::command]
fn set_settings(shell: State<'_, Shell>, settings: SettingsDto) -> Result<SettingsDto, ErrorDto> {
    shell.set_settings(&settings)
}

#[tauri::command]
fn get_thresholds(shell: State<'_, Shell>) -> ThresholdsDto {
    shell.thresholds()
}

pub fn run() {
    tauri::Builder::default()
        .manage(Shell::mock())
        .invoke_handler(tauri::generate_handler![
            get_state,
            start_scan,
            list_cards,
            decide_cards,
            undo,
            decide_cards_confirmed,
            protect,
            list_protections,
            list_rules,
            save_rule,
            preview_rule,
            toggle_rule,
            delete_rule,
            restore_rule,
            list_people,
            name_person,
            merge_people,
            unmerge_person,
            protect_person,
            get_face_index,
            set_face_index,
            wipe_face_index,
            get_dashboard,
            compare_series,
            get_settings,
            set_settings,
            get_thresholds
        ])
        .run(tauri::generate_context!())
        .expect("kunne ikke starte Fjell");
}
