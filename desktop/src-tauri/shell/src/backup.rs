//! Sikkerhetskopi av indeksen, integritet, trygg modus og gjenoppretting for skallet (`impl Shell`). Del av `fjell-shell`.
//! Se docs/v1/BACKUP.md og docs/v1/ROBUSTHET-KONTRAKT.md.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`, `Fjell::backup_auto_if_due`), `fjell-core::store::Store` (filbaserte
//! kall for trygg modus uten åpen indeks), `dto_robust`, `config`. Regler (CLAUDE.md): kopien inneholder bare
//! indeksen, aldri bilder. Gjenoppretting krever `RestoreAck` (avkrysning) og er angrebar (`backup_restore_undo`).
//! Ingen bildefiler leses eller skrives her; filarbeidet på indeksen ligger i kjernen (`store::backup`).
//!
//! Trygg modus (indeksen lar seg ikke åpne ved oppstart, beslutning i docs/v1/BACKUP.md):
//! - `integrity()` sjekker indeksfilen direkte (rask `quick_check`) og svarer `corrupt`/`unreadable`;
//! - `backups()`, `backup_restore()` og `backup_restore_undo()` virker uten åpen kjerne og åpner biblioteket
//!   igjen etterpå (karantenen avstemmes da som ved all åpning, beslutning 20);
//! - `rebuild_index()` flytter den ødelagte indeksen til siden (slettes aldri), åpner en ny og starter skanning.
//! Gjenoppretting av en indeks som ikke kunne åpnes har ingen «før»-kopi å angre til; den ødelagte filen
//! ligger igjen som `<indeks>.corrupt-<tid>` i app-data-mappen (`safety` = 0 i svaret).

use crate::config;
use crate::dto::ErrorDto;
use crate::dto_robust::{BackupDto, RestoreReportDto};
use crate::{Inner, Shell};
use fjell_core::api::FjellApi;
use fjell_core::model::{BackupId, BackupKind, RestoreAck};
use fjell_core::store::Store;
use fjell_fs::roots::PRIMARY_KEY;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Resultat av «bygg indeksen på nytt».
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RebuildDto {
    /// Den ødelagte indeksen er bevart ved siden av (aldri slettet).
    pub corrupt_kept: bool,
    /// Skanning av mappene er startet (bygger indeksen opp igjen).
    pub scanning: bool,
    /// «Nylig slettet» og angrehistorikk fra den gamle indeksen er borte. Filer som lå i karantenen er
    /// lagt tilbake på opprinnelig sted ved avstemmingen; ingen fil er slettet. Brukeren skal alltid se dette.
    pub trash_state_reset: bool,
}

/// Resultat av forberedelsen før indeksen åpnes ved oppstart.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDto {
    /// `none`, `rolled_back` (avbrutt gjenoppretting satt tilbake) eller `cleaned_up`.
    pub recovered: &'static str,
    /// En automatisk kopi ble tatt fordi skjemaet skal migreres.
    pub migration_backup: bool,
}

fn core_error(code: &'static str) -> ErrorDto {
    ErrorDto::new("core_error", code)
}

fn no_library(e: &ErrorDto) -> bool {
    e.code == "no_library"
}

fn dto_of(m: &fjell_core::api::BackupEntry) -> BackupDto {
    BackupDto::from(m)
}

impl Shell {
    pub fn backup_create(&self) -> Result<BackupDto, ErrorDto> {
        let e = self.api()?.backup_create()?;
        Ok(BackupDto::from(&e))
    }

    /// Kopiene, nyeste først. Uten åpen indeks (trygg modus) leses de direkte fra kopimappen.
    pub fn backups(&self) -> Result<Vec<BackupDto>, ErrorDto> {
        match self.api() {
            Ok(api) => Ok(api.backup_list()?.iter().map(dto_of).collect()),
            Err(e) if no_library(&e) => Ok(self
                .closed_db()
                .map(|db| {
                    Store::backup_list_at(&db)
                        .iter()
                        .map(|m| BackupDto {
                            id: m.id.0,
                            kind: m.kind.as_str(),
                            created_at: m.created_at,
                            bytes: m.bytes,
                            asset_count: m.asset_count,
                            integrity: m.integrity.as_str(),
                        })
                        .collect()
                })
                .unwrap_or_default()),
            Err(e) => Err(e),
        }
    }

    /// Gjenoppretter `id`. `acknowledged` er brukerens avkrysning; uten den sendes `backup_ack_required`
    /// og ingenting røres. Etter byttet avstemmes karantenen mot den gjenopprettede indeksen (filer som ikke
    /// lenger er «slettet» der legges tilbake; ingen fil slettes).
    pub fn backup_restore(
        &self,
        id: u64,
        acknowledged: bool,
    ) -> Result<RestoreReportDto, ErrorDto> {
        let ack = RestoreAck::new(BackupId(id), acknowledged)
            .ok_or_else(|| core_error("backup_ack_required"))?;
        match self.parts() {
            Ok((api, core)) => {
                // Ingen sletting, angre eller «Tøm» midt i byttet: flyt-låsen holdes til indeksen er byttet.
                let flow = core.as_ref().map(|c| c.flow());
                let r = api.backup_restore(ack);
                drop(flow);
                let r = r?;
                if let Some(c) = &core {
                    c.reconcile();
                }
                Ok(RestoreReportDto::from(&r))
            }
            Err(e) if no_library(&e) => self.restore_closed(ack.backup(), false),
            Err(e) => Err(e),
        }
    }

    /// Angrer en gjenoppretting (`safety` fra `RestoreReportDto`).
    pub fn backup_restore_undo(&self, safety: u64) -> Result<RestoreReportDto, ErrorDto> {
        match self.parts() {
            Ok((api, core)) => {
                let flow = core.as_ref().map(|c| c.flow());
                let r = api.backup_restore_undo(BackupId(safety));
                drop(flow);
                let r = r?;
                if let Some(c) = &core {
                    c.reconcile();
                }
                Ok(RestoreReportDto::from(&r))
            }
            Err(e) if no_library(&e) => self.restore_closed(BackupId(safety), true),
            Err(e) => Err(e),
        }
    }

    /// Integritet som kode (`ok`, `degraded`, `corrupt`, `unreadable`). Uten åpen kjerne sjekkes indeksfilen
    /// til sist brukte bibliotek direkte (rask sjekk): en indeks som ikke lot seg åpne gir `corrupt` eller
    /// `unreadable`, ikke `ok`. Uten noe bibliotek (første oppstart) finnes ingenting å sjekke: `ok`.
    pub fn integrity(&self) -> Result<&'static str, ErrorDto> {
        match self.api() {
            Ok(api) => Ok(api.integrity_check()?.as_str()),
            Err(e) if no_library(&e) => Ok(self
                .closed_db()
                .map_or("ok", |db| Store::check_file(&db, false).status().as_str())),
            Err(e) => Err(e),
        }
    }

    /// Automatisk kopi ved et trygt tidspunkt (`weekly` eller `before_purge`), bare hvis den trengs.
    /// Kalles fra en bakgrunnstråd i appen, aldri fra en UI-handling. Mock og uten bibliotek: `None`.
    pub fn backup_auto(&self, reason: &str) -> Result<Option<BackupDto>, ErrorDto> {
        let core = match &*self.read() {
            Inner::Core(c) => c.clone(),
            _ => return Ok(None),
        };
        Ok(core
            .fjell
            .backup_auto_if_due(reason)?
            .as_ref()
            .map(BackupDto::from))
    }

    /// Før indeksen `db` åpnes ved oppstart: setter en avbrutt gjenoppretting tilbake (krasj midt i byttet) og
    /// tar en automatisk kopi hvis skjemaet skal migreres. Kalles før `Store::open`. Feil her stopper ikke
    /// oppstarten (kopien er en ekstra sikring, ikke et vilkår).
    pub fn prepare_index(&self, db: &Path) -> PrepareDto {
        let now = self.clock.now();
        let recovered = Store::recover_interrupted_restore(db, now).code();
        let migration_backup = matches!(Store::backup_before_migration_at(db, now), Ok(Some(_)));
        PrepareDto {
            recovered,
            migration_backup,
        }
    }

    /// «Bygg indeksen på nytt»: den ødelagte indeksen flyttes til siden (aldri slettet), en ny åpnes og mappene
    /// skannes. Originaler og karantene røres ikke; karantenen avstemmes ved åpningen (beslutning 20). Avvises
    /// (`bad_request`) hvis indeksen er hel, så en frisk indeks aldri kastes ved en feil.
    pub fn rebuild_index(&self) -> Result<RebuildDto, ErrorDto> {
        let dir = self.data_dir()?.clone();
        let lib = config::last_library(&dir).ok_or_else(|| ErrorDto::new("no_library", ""))?;
        let db = config::locate_index(&dir, &lib, Some(&lib));
        if db.exists() && Store::check_file(&db, true).status().is_ok() {
            return Err(ErrorDto::bad_request("indeksen er hel"));
        }
        self.close_core();
        let mut kept = false;
        if db.exists() {
            Store::quarantine_corrupt_index(&db, self.clock.now())
                .map_err(|e| core_error(e.code()))?;
            kept = true;
        }
        self.reopen_last()?;
        let scanning = self.start_scan(true).is_ok();
        Ok(RebuildDto {
            corrupt_kept: kept,
            scanning,
            trash_state_reset: true,
        })
    }

    // ---- hjelpere ----

    /// Indeksfilen til sist brukte bibliotek, hvis den finnes (trygg modus uten åpen kjerne).
    fn closed_db(&self) -> Option<PathBuf> {
        let dir = self.data_dir.as_ref()?;
        let lib = config::last_library(dir)?;
        let db = config::locate_index(dir, &lib, Some(&lib));
        db.exists().then_some(db)
    }

    /// Gjenoppretter uten åpen kjerne og åpner biblioteket igjen. `undo`: bare `PreRestore`-kopier.
    fn restore_closed(&self, id: BackupId, undo: bool) -> Result<RestoreReportDto, ErrorDto> {
        let db = self
            .closed_db()
            .ok_or_else(|| ErrorDto::new("no_library", ""))?;
        let now = self.clock.now();
        if undo
            && !Store::backup_list_at(&db)
                .iter()
                .any(|m| m.id == id && m.kind == BackupKind::PreRestore)
        {
            return Err(core_error("backup_not_found"));
        }
        let m = Store::restore_closed_at(&db, id, now).map_err(|e| core_error(e.code()))?;
        self.reopen_last()?;
        Ok(RestoreReportDto {
            restored: id.0,
            safety: 0,
            asset_count: m.asset_count,
        })
    }

    /// Åpner sist brukte bibliotek (med lagrede ekstra mapper) som ved oppstart.
    fn reopen_last(&self) -> Result<(), ErrorDto> {
        let dir = self.data_dir()?.clone();
        let lib = config::last_library(&dir).ok_or_else(|| ErrorDto::new("no_library", ""))?;
        let mut roots = vec![(PRIMARY_KEY, lib.clone())];
        roots.extend(config::extra_roots(&dir, &lib));
        self.open_production(&roots, &[])
    }

    /// Lukker den åpne kjernen (venter på pågående flyt og skanning) så indeksfilen kan flyttes.
    fn close_core(&self) {
        let old = match &*self.read() {
            Inner::Core(c) => Some(c.clone()),
            _ => None,
        };
        if let Some(old) = old {
            let flow = old.flow();
            old.fjell.cancel();
            let _ = old.fjell.wait_scan();
            old.retire(&flow);
            drop(flow);
            *self.inner.write().unwrap_or_else(|e| e.into_inner()) = Inner::NoLibrary;
        }
    }
}
