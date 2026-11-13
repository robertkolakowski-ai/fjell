# Beslutninger fra red team

## F1: gjenoppretting ved navnekollisjon (utsatt, testen står ignorert)

Problem: `unstage` gir `navn (gjenopprettet).ext`, men skallet (`Core::restore_undone`,
`desktop/src-tauri/shell/src/core.rs`) kaster den nye `SourceId`, så indeksen peker på den gamle stien.

Forslag (krever kode utenfor denne fasens filer):
1. Ny skrivevei i kjernen (`Store`/`Actions`, f.eks. `relocate_source(id, SourceId)`) som oppdaterer `asset.source_id`.
2. `restore_undone` kaller den med resultatet fra `unstage` (bare `Ok`).
3. Testhjelperen `restore_undone` i `core/crates/fjell-cli/tests/redteam_fable.rs` speiler skallet og kaster også
   resultatet; den må oppdateres likt. Testen kan derfor ikke grønnes uten å endre testfilen utover `#[ignore]`.

Brukersynlig: ingen endring i normal flyt; ved kollisjon peker kortet på riktig fil. Avklar om testhjelperen får endres.
