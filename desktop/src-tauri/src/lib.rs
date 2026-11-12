//! Tauri-limet for Fjell desktop: registrerer kommandoer som delegerer til `fjell-shell`.
//!
//! Avhenger av: `tauri`, `tauri-plugin-dialog`, `fjell-shell`.
//! Grensesnitt: `run()` (kalles fra `main.rs`). Kommandoer: `get_state`, `start_scan`,
//! `cancel_scan`, `list_cards`, `decide_cards`, `set_keeper`, `undo`, `trash_summary`,
//! `empty_trash`, `set_library`, `get_library`, `pause_scan`, `resume_scan`, `list_trashed`,
//! `restore_trashed`, `restore_all_trashed`, `get_settings`, `set_trash_days`, `get_first_run`,
//! `complete_onboarding`, `get_language`, `set_language`. Ingen logikk her; ingen sletting utenom
//! `Shell::empty` (Papirkurv som standard, beslutning 17).
//!
//! Alle kommandoer er `async` slik at filarbeid ikke kjører på hovedtråden (< 100 ms-målet);
//! de tyngste (`set_library`, `empty_trash`) kjører i `spawn_blocking`.
//! Mappevelger: `tauri-plugin-dialog` (bare `dialog:allow-open`, se capabilities/default.json) kalles fra
//! frontend (`pickFolder`) og fyller bare stifeltet; `set_library` tar fortsatt en sti, og brukeren bekrefter.
//! IKKE bygget eller kjørt i utviklingsmiljøet (krever webkit2gtk/WebView2).

use fjell_shell::dto::{
    CardPageDto, DecisionDto, EmptyDto, ErrorDto, PermanentAckDto, RestoreDto, SettingsDto,
    StateDto, TrashSummaryDto, TrashedDto,
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

#[tauri::command]
async fn get_language(shell: State<'_, Arc<Shell>>) -> Res<Option<String>> {
    Ok(shell.language())
}

#[tauri::command]
async fn set_language(shell: State<'_, Arc<Shell>>, language: String) -> Res<String> {
    shell.set_language(&language)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
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
            pause_scan,
            resume_scan,
            list_trashed,
            restore_trashed,
            restore_all_trashed,
            get_settings,
            set_trash_days,
            get_first_run,
            complete_onboarding,
            get_language,
            set_language
        ])
        .run(tauri::generate_context!())
        .expect("kunne ikke starte Fjell");
}
