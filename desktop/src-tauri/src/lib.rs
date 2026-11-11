//! Tauri-limet for Fjell desktop: registrerer kommandoer som delegerer til `fjell-shell`.
//!
//! Avhenger av: `tauri`, `fjell-shell`.
//! Grensesnitt: `run()` (kalles fra `main.rs`). Kommandoer: `get_state`, `start_scan`,
//! `list_cards`, `decide_cards`, `undo`. Ingen logikk her; ingen sletting (beslutning 17).

use fjell_shell::dto::{CardPageDto, DecisionDto, ErrorDto, StateDto};
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

pub fn run() {
    tauri::Builder::default()
        .manage(Shell::mock())
        .invoke_handler(tauri::generate_handler![
            get_state,
            start_scan,
            list_cards,
            decide_cards,
            undo
        ])
        .run(tauri::generate_context!())
        .expect("kunne ikke starte Fjell");
}
