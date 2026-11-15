//! Integrasjonstester mot skallets offentlige flate (`fjell_shell::Shell`). Hver ny V1-fase legger
//! egne filer i denne mappen (`tests/<fase>.rs`); Cargo oppdager dem uten endring i delte filer
//! (`Cargo.toml`, `lib.rs`). Denne filen er bare kontraktens smoketest.

use fjell_shell::Shell;

#[test]
fn mock_shell_starts_without_library() {
    let shell = Shell::mock();
    assert!(shell.library().is_none());
}
