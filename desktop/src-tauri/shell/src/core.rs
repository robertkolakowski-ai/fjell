//! Ekte kjerne for ett bibliotek: `Fjell` + `FsSource` + indeks i app-data + karantene + «Tøm».
//!
//! Avhenger av: `fjell-core::api` (`Fjell`), `fjell-fs` (`FsQuarantine`, `FsDeleter`, `RecycleBin`).
//! Grensesnitt: `Core::open`, `flow`, `stage_trashed`, `undo`, `restore`, `reconcile`, `empty`.
//!
//! Regler (CLAUDE.md, ARKITEKTUR 17/20/22):
//! - Trash er indeks + flytting til karantene (rename); undo flytter tilbake. Aldri sletting.
//! - «Tøm» går bare via `begin_purge` → `confirm_purge` → Papirkurv (standard) → `finish_purge`.
//!   Permanent krever `PermanentAck` (avkrysning + riktig antall), og `Unsupported` volum faller
//!   aldri stille tilbake til permanent.
//! - Rekkefølge: indeksen først, filflytting etterpå. Brudd midt imellom rettes av `reconcile`.
//! - Invariant (F1/F2/F4): «Tøm» sletter bare filer som på Tøm-tidspunktet er i «Nylig slettet» OG som
//!   karantenen faktisk inneholder. Derfor (1) holder hver sammensatt overgang (trash + flytting,
//!   angre, gjenoppretting, «Tøm», avstemming) `flow`-låsen fra indeksendring til filflytting er
//!   ferdig, (2) følger indeksen filen når karantenen gjenoppretter til et annet navn (`relocate`),
//!   og (3) legges filen tilbake i karantenen når en gjenoppretting angres (`restage_trashed`).

use crate::dto::{EmptyDto, ErrorDto, PermanentAckDto, RestoreDto, TrashedDto};
use fjell_core::api::{Fjell, FjellApi};
use fjell_core::model::{
    AssetId, AssetSource, BatchId, Clock, ConsequenceCode, DeleteOutcome, SourceId, State,
};
use fjell_core::store::Store;
use fjell_fs::{
    DeleteMode, DeleteResult, FsDeleter, FsQuarantine, PermanentAck, Quarantine, Reconciled,
    RecycleBin,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub(crate) struct Core {
    pub fjell: Arc<Fjell>,
    pub library: PathBuf,
    /// Indeksfilen denne kjernen har åpen (kan være gammelt navn etter mislykket migrering).
    pub db: PathBuf,
    quarantine: Arc<FsQuarantine>,
    /// Én om gangen av overgangene som endrer både indeks og karantene (se invarianten øverst).
    flow: Mutex<()>,
}

/// Mappen i en `SourceId` (alt før siste `/`).
fn folder(s: &SourceId) -> &str {
    s.0.rsplit_once('/').map_or("", |x| x.0)
}

/// Siste sti-ledd (Windows- og Unix-skille). Aldri hele stien til frontend.
fn file_name(source: &str) -> String {
    source
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source)
        .to_string()
}

fn internal(e: impl std::fmt::Display) -> ErrorDto {
    ErrorDto::new("internal", e.to_string())
}

impl Core {
    /// Åpner indeksen på `db` (må ligge utenfor biblioteket) og avstemmer karantenen.
    pub fn open(
        library: &Path,
        db: &Path,
        source: Arc<dyn AssetSource>,
        clock: Arc<dyn Clock>,
    ) -> Result<Core, ErrorDto> {
        if !library.is_dir() {
            return Err(ErrorDto::new("not_a_folder", library.display().to_string()));
        }
        if db.starts_with(library) {
            return Err(ErrorDto::bad_request(
                "indeksen må ligge utenfor biblioteket",
            ));
        }
        let store = Store::open(db).map_err(internal)?;
        let core = Core {
            fjell: Arc::new(Fjell::new(store, source, clock)),
            library: library.to_path_buf(),
            db: db.to_path_buf(),
            quarantine: Arc::new(FsQuarantine::in_library(library)),
            flow: Mutex::new(()),
        };
        core.reconcile();
        Ok(core)
    }

    /// Alle id-er i «Nylig slettet». Leser indeksen med egen lesetilkobling; faller tilbake til
    /// det som ligger i karantene hvis lesing ikke er mulig.
    /// Låsen for sammensatte overganger. Tas FØR indeksen endres og holdes til filene er flyttet.
    pub fn flow(&self) -> MutexGuard<'_, ()> {
        self.flow.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn trashed_ids(&self) -> Vec<AssetId> {
        let from_db = self.fjell.with_store(|s| {
            let conn = s.reader().ok().flatten()?;
            let mut st = conn
                .prepare("SELECT id FROM asset WHERE state = 1 ORDER BY id")
                .ok()?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0)).ok()?;
            Some(
                rows.filter_map(|r| r.ok())
                    .map(|i| AssetId(i as u64))
                    .collect(),
            )
        });
        from_db.unwrap_or_else(|| self.staged_still_trashed())
    }

    fn staged_still_trashed(&self) -> Vec<AssetId> {
        let ids: Vec<AssetId> = self.quarantine.staged().into_iter().map(|s| s.0).collect();
        let rows = self
            .fjell
            .with_store(|s| s.asset_rows(&ids))
            .unwrap_or_default();
        ids.into_iter()
            .zip(rows)
            .filter(|(_, r)| r.as_ref().is_some_and(|r| r.state == State::Trashed))
            .map(|(i, _)| i)
            .collect()
    }

    /// Karantenemappen (`<bibliotek>/.fjell-karantene`). Vises i Innstillinger; ingenting leses fra frontend.
    pub fn quarantine_dir(&self) -> PathBuf {
        self.library.join(fjell_fs::QUARANTINE_DIR)
    }

    /// Alle bilder i «Nylig slettet», eldste først. `now` brukes bare til `expired`.
    pub fn trashed(&self, now: i64) -> Vec<TrashedDto> {
        let ids = self.trashed_ids();
        let rows = self
            .fjell
            .with_store(|s| s.asset_rows(&ids))
            .unwrap_or_default();
        let sources = self.fjell.sources_for(&ids);
        let mut out: Vec<TrashedDto> = rows
            .into_iter()
            .flatten()
            .filter(|r| r.state == State::Trashed)
            .map(|r| {
                let name = sources
                    .iter()
                    .find(|(id, _)| *id == r.id)
                    .map(|(_, src)| file_name(&src.0))
                    .unwrap_or_default();
                let expires_at = r.expires_at.unwrap_or(i64::MAX);
                TrashedDto {
                    asset: r.id.0,
                    name,
                    bytes: r.bytes,
                    trashed_at: r.trashed_at.unwrap_or(0),
                    expires_at,
                    expired: expires_at <= now,
                }
            })
            .collect();
        out.sort_by_key(|t| (t.trashed_at, t.asset));
        out
    }

    /// Gjenoppretter `ids` (bare de som ligger i «Nylig slettet» telles). Indeksen først, så flyttes
    /// filene tilbake fra karantenen. Feil ved flytting lar filen ligge; `reconcile` retter det ved oppstart.
    pub fn restore(&self, ids: &[u64]) -> Result<RestoreDto, ErrorDto> {
        let _flow = self.flow();
        let trashed = self.trashed_ids();
        let wanted: Vec<AssetId> = ids
            .iter()
            .map(|i| AssetId(*i))
            .filter(|a| trashed.contains(a))
            .collect();
        if wanted.is_empty() {
            return Ok(RestoreDto::default());
        }
        self.fjell.restore_assets(&wanted).map_err(ErrorDto::from)?;
        self.unstage_active();
        Ok(RestoreDto {
            restored: wanted.len() as u64,
        })
    }

    pub fn restore_all(&self) -> Result<RestoreDto, ErrorDto> {
        let all: Vec<u64> = self.trashed_ids().into_iter().map(|a| a.0).collect();
        self.restore(&all)
    }

    /// Oppstart: filer i karantene som ikke lenger er `trashed` flyttes tilbake (avbrutt angre).
    pub fn reconcile(&self) {
        let _flow = self.flow();
        let ids = self.trashed_ids();
        let trashed = self.fjell.sources_for(&ids);
        for (id, r) in self.quarantine.reconcile(&trashed) {
            if let Reconciled::Restored(now) = r {
                self.relocate(id, &now);
            }
        }
    }

    /// Karantenen la filen på `now`. Er det et annet navn i samme mappe enn indeksen har
    /// (navnekollisjon), pekes indeksen dit; ellers ville neste trash av kortet flytte, og «Tøm»
    /// slette, den fremmede filen som ligger på den opprinnelige stien (F1).
    fn relocate(&self, id: AssetId, now: &SourceId) {
        let old = self.fjell.sources_for(&[id]);
        let Some((_, old)) = old.first() else { return };
        if old != now && folder(old) == folder(now) {
            let _ = self.fjell.relocate_source(id, now);
        }
    }

    /// Etter vellykket trash: flytt filene til karantene (batch-id fra kjernen). Feil (annet volum,
    /// frakoblet) lar filen ligge: den er da «Merket, ikke flyttet» og originalen er urørt.
    /// Kalleren holder `flow` fra før indeksen ble endret.
    pub fn stage_trashed(&self, batch: BatchId, assets: &[AssetId]) {
        let items = self.fjell.sources_for(assets);
        let _ = self.quarantine.stage(batch, &items);
    }

    /// Angre siste batch (indeks), og bring karantenen i samsvar begge veier: angret trash flyttes
    /// tilbake, og angret gjenoppretting legges tilbake i karantenen (F4). Gir angret batch-id.
    pub fn undo(&self) -> Result<u64, ErrorDto> {
        let _flow = self.flow();
        let batch = self.fjell.undo().map_err(ErrorDto::from)?.batch;
        self.unstage_active();
        self.restage_trashed(batch);
        Ok(batch.0)
    }

    /// Alt som er `trashed` i indeksen, men ikke ligger i karantenen, flyttes dit (samme regler som
    /// `stage_trashed`: feil lar filen ligge urørt som «Merket, ikke flyttet»).
    fn restage_trashed(&self, batch: BatchId) {
        let staged: Vec<AssetId> = self.quarantine.staged().into_iter().map(|s| s.0).collect();
        let missing: Vec<AssetId> = self
            .trashed_ids()
            .into_iter()
            .filter(|a| !staged.contains(a))
            .collect();
        if !missing.is_empty() {
            self.stage_trashed(batch, &missing);
        }
    }

    /// Alt i karantene som ikke lenger er `trashed` flyttes tilbake. Kalleren holder `flow`.
    fn unstage_active(&self) {
        let ids: Vec<AssetId> = self.quarantine.staged().into_iter().map(|s| s.0).collect();
        let rows = self
            .fjell
            .with_store(|s| s.asset_rows(&ids))
            .unwrap_or_default();
        let back: Vec<AssetId> = ids
            .into_iter()
            .zip(rows)
            .filter(|(_, r)| r.as_ref().is_some_and(|r| r.state == State::Active))
            .map(|(i, _)| i)
            .collect();
        if !back.is_empty() {
            for (id, r) in self.quarantine.unstage(&back) {
                if let Ok(now) = r {
                    self.relocate(id, &now);
                }
            }
        }
    }

    /// «Tøm» alt i «Nylig slettet». `confirmed` er konsekvenskoder brukeren har godtatt.
    pub fn empty(
        &self,
        bin: Arc<dyn RecycleBin>,
        confirmed: Vec<ConsequenceCode>,
        permanent: Option<PermanentAckDto>,
    ) -> Result<EmptyDto, ErrorDto> {
        let (mode, ack) = match permanent {
            None => (DeleteMode::Recycle, None),
            Some(p) => match PermanentAck::new(p.checked, p.count) {
                Some(a) => (DeleteMode::Permanent, Some((a, p.count))),
                None => return Err(ErrorDto::new("ack_required", "permanent")),
            },
        };
        // Låsen holdes fra billetten lages til siste fil er behandlet: ingen gjenoppretting eller
        // angre kan flytte en fil mellom bekreftelsen og slettingen (F2).
        let _flow = self.flow();
        let ids = self.trashed_ids();
        if ids.is_empty() {
            return Ok(EmptyDto::default());
        }
        let intent = self.fjell.begin_purge(ids).map_err(ErrorDto::from)?;
        if let Some((_, n)) = ack {
            if n != intent.ids.len() {
                return Err(ErrorDto::new("ack_mismatch", "permanent"));
            }
        }
        let deleter = FsDeleter::new(
            &self.library,
            Some(self.quarantine.clone()),
            bin,
            mode,
            ack.map(|a| a.0),
        )?;
        let before = self.fjell.trash_summary().freed_bytes;
        let ticket = self
            .fjell
            .confirm_purge_confirmed(&intent, confirmed)
            .map_err(ErrorDto::from)?;
        let sources: Vec<(AssetId, SourceId)> = self.fjell.sources_for(ticket.ids());
        let detailed = deleter.delete_detailed(&ticket, &sources)?;
        let mut out = EmptyDto::default();
        let outcomes: Vec<(AssetId, DeleteOutcome)> = detailed
            .into_iter()
            .map(|(id, r)| {
                let o = match r {
                    DeleteResult::Deleted => DeleteOutcome::Deleted,
                    DeleteResult::Cancelled => {
                        out.cancelled += 1;
                        DeleteOutcome::Cancelled
                    }
                    DeleteResult::Unsupported => {
                        out.unsupported += 1;
                        DeleteOutcome::Failed
                    }
                    DeleteResult::Unavailable => {
                        out.unavailable += 1;
                        DeleteOutcome::Failed
                    }
                    DeleteResult::Failed => DeleteOutcome::Failed,
                };
                (id, o)
            })
            .collect();
        let rep = self
            .fjell
            .finish_purge(ticket, outcomes)
            .map_err(ErrorDto::from)?;
        out.purged = rep.purged.len() as u64;
        out.kept_trashed = rep.kept_trashed.len() as u64;
        out.freed_bytes = self
            .fjell
            .trash_summary()
            .freed_bytes
            .saturating_sub(before);
        Ok(out)
    }
}
