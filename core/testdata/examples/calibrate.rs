//! `cargo run --release --locked --example calibrate`: feier terskler over det syntetiske korpuset og
//! skriver presisjon/recall per sensitivitet (Low/Normal/High) som Markdown på stdout.
//! Samme tekst som står i docs/v1/KALIBRERING-V1.md (testen `calibration_doc_is_current` sjekker det).
//! Syntetisk korpus er ikke ekte bilder: se «Begrensninger» i dokumentet.

#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    print!("{}", common::report());
}
