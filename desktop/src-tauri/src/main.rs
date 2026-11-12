//! Inngang for Fjell desktop. Skjuler konsollvindu i release på Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fjell_desktop_lib::run()
}
