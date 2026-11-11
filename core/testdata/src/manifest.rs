//! Grensesnitt: `Entry`, `Manifest { seed, algo_note, entries }`, `Manifest::to_json`.
//! Fasit for golden-korpuset. `exact_group`/`visual_group`: None = ingen gruppe.
//! `blurry`/`screenshot`: None = ikke relevant for fila. Ingen klokkeslett, ingen sti
//! utenfor korpus-mappa, så to kjøringer gir identisk JSON.

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Entry {
    pub file: String,
    pub thumb: String,
    pub group: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub fnv64: String,
    pub created_at: i64,
    pub exact_group: Option<u32>,
    pub visual_group: Option<u32>,
    pub blurry: Option<bool>,
    pub screenshot: Option<bool>,
    pub platform_screenshot_hint: bool,
    pub has_camera_exif: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct Manifest {
    pub seed: u64,
    pub thumb_note: &'static str,
    pub entries: Vec<Entry>,
}

impl Manifest {
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("json");
        s.push('\n');
        s
    }
}
