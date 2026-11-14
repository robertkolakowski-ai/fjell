//! Tauri-limet for Fjell desktop: registrerer kommandoer som delegerer til `fjell-shell`.
//!
//! Avhenger av: `tauri`, `tauri-plugin-dialog`, `tauri-plugin-process`, `tauri-plugin-updater`, `fjell-shell`.
//! Grensesnitt: `run()` (kalles fra `main.rs`). Kommandoer: `get_state`, `start_scan`,
//! `cancel_scan`, `list_cards`, `decide_cards`, `set_keeper`, `undo`, `trash_summary`,
//! `empty_trash`, `set_library`, `get_library`, `list_library_roots`, `add_library_root`, `remove_library_root`, `pause_scan`, `resume_scan`, `list_trashed`,
//! `restore_trashed`, `restore_all_trashed`, `get_thumbnail`, `get_settings`, `set_trash_days`, `get_first_run`,
//! `complete_onboarding`, `get_auto_update`, `set_auto_update`, `get_update_download_url`, `get_language`, `set_language`.
//! V1 (ingen skjermer bruker dem ennå): `protect`, `list_protections`, `list_rules`, `save_rule`, `preview_rule`,
//! `toggle_rule`, `delete_rule`, `restore_rule`, `list_people`, `name_person`, `merge_people`, `unmerge_person`,
//! `protect_person`, `get_face_index`, `set_face_index`, `wipe_face_index`, `get_dashboard`, `compare_series`,
//! `get_suggestion_settings`, `set_suggestion_settings`, `get_thresholds`. Plugins: `dialog` (mappevalg),
//! `updater` (bare skrivebord) og `process` (omstart etter oppdatering); sjekk/installasjon kalles fra frontend. Ingen logikk her; ingen sletting utenom
//! `Shell::empty` (Papirkurv som standard, beslutning 17).
//!
//! Alle kommandoer er `async` slik at filarbeid ikke kjører på hovedtråden (< 100 ms-målet);
//! de tyngste (`set_library`, `empty_trash`) kjører i `spawn_blocking`.
//! Mappevelger: `tauri-plugin-dialog` (bare `dialog:allow-open`, se capabilities/default.json) kalles fra
//! frontend (`pickFolder`) og fyller bare stifeltet; `set_library` tar fortsatt en sti, og brukeren bekrefter.
//! IKKE bygget eller kjørt i utviklingsmiljøet (krever webkit2gtk/WebView2).

use fjell_shell::dto::{
    CardPageDto, DecisionDto, EmptyDto, ErrorDto, PermanentAckDto, RestoreDto, RootDto,
    SettingsDto, StateDto, ThumbDto, TrashSummaryDto, TrashedDto,
};
use fjell_shell::dto_v1::{
    ConditionDto, DashboardDto, PersonDto, ProtectionDto, RuleDraftDto, RuleDto, RulePreviewDto,
    ScopeDto, SeriesCompareDto, SuggestionSettingsDto, ThresholdsDto,
};
use fjell_shell::{default_recycle_bin, Shell};
use std::sync::Arc;
use tauri::{Manager, State};

type Res<T> = Result<T, ErrorDto>;

#[tauri::command]
async fn get_state(shell: State<'_, Arc<Shell>>) -> Res<StateDto> {
    Ok(shell.state())
}

#[tauri::command]
async fn start_scan(shell: State<'_, Arc<Shell>>, full: bool) -> Res<()> {
    shell.start_scan(full)
}

#[tauri::command]
async fn cancel_scan(shell: State<'_, Arc<Shell>>) -> Res<()> {
    shell.cancel_scan()
}

#[tauri::command]
async fn list_cards(
    shell: State<'_, Arc<Shell>>,
    kind: String,
    limit: usize,
    cursor: Option<u64>,
) -> Res<CardPageDto> {
    shell.cards(&kind, limit, cursor)
}

/// `confirmed` er valgfri: koder fra en tidligere `ack_required` som brukeren har bekreftet.
#[tauri::command]
async fn decide_cards(
    shell: State<'_, Arc<Shell>>,
    ids: Vec<u64>,
    decision: DecisionDto,
    confirmed: Option<Vec<String>>,
) -> Res<u64> {
    shell.decide_confirmed(&ids, decision, &confirmed.unwrap_or_default())
}

#[tauri::command]
async fn set_keeper(shell: State<'_, Arc<Shell>>, group: u64, keeper: u64) -> Res<()> {
    shell.set_keeper(group, keeper)
}

#[tauri::command]
async fn undo(shell: State<'_, Arc<Shell>>) -> Res<u64> {
    shell.undo()
}

#[tauri::command]
async fn trash_summary(shell: State<'_, Arc<Shell>>) -> Res<TrashSummaryDto> {
    shell.trash_summary()
}

/// «Tøm»: Papirkurv som standard. `permanent` (avkrysning + antall) gir permanent sletting.
#[tauri::command]
async fn empty_trash(
    shell: State<'_, Arc<Shell>>,
    confirmed: Option<Vec<String>>,
    permanent: Option<PermanentAckDto>,
) -> Res<EmptyDto> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        sh.empty(&confirmed.unwrap_or_default(), permanent)
    })
    .await
    .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

#[tauri::command]
async fn set_library(shell: State<'_, Arc<Shell>>, path: String) -> Res<StateDto> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.set_library(&path))
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

#[tauri::command]
async fn get_library(shell: State<'_, Arc<Shell>>) -> Res<Option<String>> {
    Ok(shell.library())
}

/// Bibliotekets mapper med tall per mappe (leser indeksen; ingen bildefiler åpnes).
#[tauri::command]
async fn list_library_roots(shell: State<'_, Arc<Shell>>) -> Res<Vec<RootDto>> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.list_library_roots())
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

/// Legger en mappe til i biblioteket (stien kommer fra mappevelgeren, som for `set_library`).
#[tauri::command]
async fn add_library_root(shell: State<'_, Arc<Shell>>, path: String) -> Res<Vec<RootDto>> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.add_library_root(&path))
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

/// Tar en mappe ut av biblioteket. Ingen filer røres; avvises hvis mappen har bilder i «Nylig slettet».
#[tauri::command]
async fn remove_library_root(shell: State<'_, Arc<Shell>>, key: u32) -> Res<Vec<RootDto>> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.remove_library_root(key))
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

#[tauri::command]
async fn pause_scan(shell: State<'_, Arc<Shell>>) -> Res<()> {
    shell.pause_scan()
}

#[tauri::command]
async fn resume_scan(shell: State<'_, Arc<Shell>>) -> Res<()> {
    shell.resume_scan()
}

#[tauri::command]
async fn list_trashed(shell: State<'_, Arc<Shell>>) -> Res<Vec<TrashedDto>> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.trashed())
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

#[tauri::command]
async fn restore_trashed(shell: State<'_, Arc<Shell>>, ids: Vec<u64>) -> Res<RestoreDto> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.restore(&ids))
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

#[tauri::command]
async fn restore_all_trashed(shell: State<'_, Arc<Shell>>) -> Res<RestoreDto> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.restore_all())
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

/// Miniatyr for `AssetId` (`CardDto.asset`/`TrashedDto.asset`, ikke kort-id). Dekoding kjører i
/// `spawn_blocking`, så hovedtråden og angre/trash aldri venter på den.
#[tauri::command]
async fn get_thumbnail(shell: State<'_, Arc<Shell>>, id: u64) -> Res<ThumbDto> {
    let sh = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sh.thumbnail(id))
        .await
        .map_err(|e| ErrorDto::new("internal", e.to_string()))?
}

#[tauri::command]
async fn get_settings(shell: State<'_, Arc<Shell>>) -> Res<SettingsDto> {
    shell.settings()
}

#[tauri::command]
async fn set_trash_days(shell: State<'_, Arc<Shell>>, days: u32) -> Res<SettingsDto> {
    shell.set_trash_days(days)
}

#[tauri::command]
async fn get_first_run(shell: State<'_, Arc<Shell>>) -> Res<bool> {
    Ok(shell.first_run())
}

#[tauri::command]
async fn complete_onboarding(shell: State<'_, Arc<Shell>>) -> Res<()> {
    shell.complete_onboarding()
}

/// «Sjekk automatisk ved oppstart» (beta-kanalen). Standard av.
#[tauri::command]
async fn get_auto_update(shell: State<'_, Arc<Shell>>) -> Res<bool> {
    Ok(shell.auto_update())
}

#[tauri::command]
async fn set_auto_update(shell: State<'_, Arc<Shell>>, on: bool) -> Res<bool> {
    shell.set_auto_update(on)
}

/// Reservevei når oppdatering i appen feiler: adressen til siste utgivelse (vises som tekst, åpnes aldri av appen).
#[tauri::command]
async fn get_update_download_url(shell: State<'_, Arc<Shell>>) -> Res<String> {
    Ok(shell.update_download_url().to_string())
}

#[tauri::command]
async fn get_language(shell: State<'_, Arc<Shell>>) -> Res<Option<String>> {
    Ok(shell.language())
}

#[tauri::command]
async fn set_language(shell: State<'_, Arc<Shell>>, language: String) -> Res<String> {
    shell.set_language(&language)
}

// ---- V1 (beskyttelse, forslagsregler, personer, dashboard, serier, forslagsinnstillinger). Tynne omslag. ----

#[tauri::command]
async fn protect(shell: State<'_, Arc<Shell>>, scope: ScopeDto, on: bool) -> Res<()> {
    shell.protect(&scope, on)
}

#[tauri::command]
async fn list_protections(shell: State<'_, Arc<Shell>>) -> Res<Vec<ProtectionDto>> {
    shell.protections()
}

#[tauri::command]
async fn list_rules(shell: State<'_, Arc<Shell>>) -> Res<Vec<RuleDto>> {
    shell.rules()
}

#[tauri::command]
async fn save_rule(shell: State<'_, Arc<Shell>>, draft: RuleDraftDto) -> Res<u64> {
    shell.save_rule(&draft)
}

#[tauri::command]
async fn preview_rule(
    shell: State<'_, Arc<Shell>>,
    conditions: Vec<ConditionDto>,
) -> Res<RulePreviewDto> {
    shell.preview_rule(&conditions)
}

#[tauri::command]
async fn toggle_rule(shell: State<'_, Arc<Shell>>, id: u64, enabled: bool) -> Res<()> {
    shell.toggle_rule(id, enabled)
}

#[tauri::command]
async fn delete_rule(shell: State<'_, Arc<Shell>>, id: u64) -> Res<()> {
    shell.delete_rule(id)
}

#[tauri::command]
async fn restore_rule(shell: State<'_, Arc<Shell>>, id: u64) -> Res<()> {
    shell.restore_rule(id)
}

#[tauri::command]
async fn list_people(shell: State<'_, Arc<Shell>>) -> Res<Vec<PersonDto>> {
    shell.people()
}

#[tauri::command]
async fn name_person(shell: State<'_, Arc<Shell>>, id: u64, name: Option<String>) -> Res<()> {
    shell.name_person(id, name)
}

#[tauri::command]
async fn merge_people(shell: State<'_, Arc<Shell>>, target: u64, absorbed: u64) -> Res<u64> {
    shell.merge_people(target, absorbed)
}

#[tauri::command]
async fn unmerge_person(shell: State<'_, Arc<Shell>>, absorbed: u64) -> Res<()> {
    shell.unmerge_person(absorbed)
}

#[tauri::command]
async fn protect_person(shell: State<'_, Arc<Shell>>, id: u64, protect: bool) -> Res<()> {
    shell.protect_person(id, protect)
}

#[tauri::command]
async fn get_face_index(shell: State<'_, Arc<Shell>>) -> Res<bool> {
    shell.face_index()
}

#[tauri::command]
async fn set_face_index(shell: State<'_, Arc<Shell>>, enabled: bool) -> Res<()> {
    shell.set_face_index(enabled)
}

#[tauri::command]
async fn wipe_face_index(shell: State<'_, Arc<Shell>>) -> Res<u64> {
    shell.wipe_face_index()
}

#[tauri::command]
async fn get_dashboard(shell: State<'_, Arc<Shell>>) -> Res<DashboardDto> {
    shell.dashboard()
}

#[tauri::command]
async fn compare_series(shell: State<'_, Arc<Shell>>, series: u64) -> Res<SeriesCompareDto> {
    shell.compare_series(series)
}

#[tauri::command]
async fn get_suggestion_settings(shell: State<'_, Arc<Shell>>) -> Res<SuggestionSettingsDto> {
    shell.suggestion_settings()
}

#[tauri::command]
async fn set_suggestion_settings(
    shell: State<'_, Arc<Shell>>,
    settings: SuggestionSettingsDto,
) -> Res<SuggestionSettingsDto> {
    shell.set_suggestion_settings(&settings)
}

#[tauri::command]
async fn get_thresholds(shell: State<'_, Arc<Shell>>) -> Res<ThresholdsDto> {
    shell.thresholds()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            let dir = app.path().app_data_dir()?;
            app.manage(Arc::new(Shell::with_data_dir(dir, default_recycle_bin())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            start_scan,
            cancel_scan,
            list_cards,
            decide_cards,
            set_keeper,
            undo,
            trash_summary,
            empty_trash,
            set_library,
            get_library,
            list_library_roots,
            add_library_root,
            remove_library_root,
            pause_scan,
            resume_scan,
            list_trashed,
            restore_trashed,
            restore_all_trashed,
            get_thumbnail,
            get_settings,
            set_trash_days,
            get_first_run,
            complete_onboarding,
            get_auto_update,
            set_auto_update,
            get_update_download_url,
            get_language,
            set_language,
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
            get_suggestion_settings,
            set_suggestion_settings,
            get_thresholds
        ])
        .run(tauri::generate_context!())
        .expect("kunne ikke starte Fjell");
}
