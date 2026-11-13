//! Ekte kjerne for ett bibliotek: `Fjell` + `FsSource` + indeks i app-data + karantene + «Tøm».
//!
//! Avhenger av: `fjell-core::api` (`Fjell`), `fjell-fs` (`FsQuarantine`, `FsDeleter`, `RecycleBin`).
//! Grensesnitt: `Core::open`, `stage_trashed`, `restore_undone`, `reconcile`, `empty`.
//!
//! Regler (CLAUDE.md, ARKITEKTUR 17/20/22):
//! - Trash er indeks + flytting til karantene (rename); undo flytter tilbake. Aldri sletting.
//! - «Tøm» går bare via `begin_purge` → `confirm_purge` → Papirkurv (standard) → `finish_purge`.
//!   Permanent krever `PermanentAck` (avkrysning + riktig antall), og `Unsupported` volum faller
//!   aldri stille tilbake til permanent.
//! - Rekkefølge: indeksen først, filflytting etterpå. Brudd midt imellom rettes av `reconcile`.

use crate::dto::{EmptyDto, ErrorDto, PermanentAckDto, RestoreDto, TrashedDto};
use crate::thumb;
use fjell_core::api::{Fjell, FjellApi};
use fjell_core::model::{
    AssetId, AssetSource, BatchId, Clock, ConsequenceCode, DeleteOutcome, SourceId, State, Thumb,
};
use fjell_core::store::Store;
use fjell_fs::locality::LocalProbe;
use fjell_fs::path::{os_path, resolve};
use fjell_fs::{
    DeleteMode, DeleteResult, FsDeleter, FsQuarantine, PermanentAck, Quarantine, RecycleBin,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) struct Core {
    pub fjell: Arc<Fjell>,
    pub library: PathBuf,
    quarantine: Arc<FsQuarantine>,
    source: Arc<dyn AssetSource>,
    /// Samme sjekk som kilden bruker for skyplassholdere; brukes bare på metadata (aldri innhold).
    probe: LocalProbe,
}

/// Siste sti-ledd (Windows- og Unix-skille). Aldri hele stien til frontend.
fn file_name(source: &str) -> String {
    source
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source)
        .to_string()
}

fn not_found() -> ErrorDto {
    ErrorDto::new("not_found", "")
}

fn internal(e: impl std::fmt::Display) -> ErrorDto {
    ErrorDto::new("internal", e.to_string())
}

impl Core {
    /// Åpner indeksen på `db` (må ligge utenfor biblioteket) og avstemmer karantenen. `probe` er
    /// plassholdersjekken (standard i produksjon; injiseres i tester på ikke-Windows).
    pub fn open_with_probe(
        library: &Path,
        db: &Path,
        source: Arc<dyn AssetSource>,
        clock: Arc<dyn Clock>,
        probe: LocalProbe,
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
            fjell: Arc::new(Fjell::new(store, source.clone(), clock)),
            library: library.to_path_buf(),
            quarantine: Arc::new(FsQuarantine::in_library(library)),
            source,
            probe,
        };
        core.reconcile();
        Ok(core)
    }

    /// Alle id-er i «Nylig slettet». Leser indeksen med egen lesetilkobling; faller tilbake til
    /// det som ligger i karantene hvis lesing ikke er mulig.
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

    /// Miniatyr for ett bilde (`id` = `AssetId`, det samme som `CardDto.asset`/`TrashedDto.asset`).
    ///
    /// Id-oversettelse: UI-ets kort-id (`CardDto.id`, `CardId`) er IKKE en `AssetId`; UI sender
    /// `card.asset` (og `card.keeper`). Papirkurvrader bruker `TrashedDto.asset` direkte.
    /// Papirkurv: miniatyrbufferen er nøklet på (rot, kilde-sti, mtime, størrelse) og treffer ikke
    /// etter flytting til karantene, så trashede bilder leses fra karantenens lagrede sti
    /// (`FsQuarantine::locate`) og dekodes uten buffer. Ikke flyttet (annet volum) leses fra
    /// originalstien via kilden. Låsen på kjernen holdes bare under indeksoppslaget, aldri under
    /// filtilgang eller dekoding, så angre/trash blokkeres ikke. Skyplassholdere leses aldri:
    /// bare metadata sjekkes. Originalfiler åpnes bare for lesing.
    pub fn thumbnail(&self, id: AssetId) -> Result<Thumb, ErrorDto> {
        let (row, src) = self
            .fjell
            .with_store(|s| (s.asset_row(id), s.source_ids(&[id])));
        let row = row.map_err(internal)?.ok_or_else(not_found)?;
        let sid = src
            .map_err(internal)?
            .into_iter()
            .next()
            .map(|(_, s)| s)
            .ok_or_else(not_found)?;
        match row.state {
            State::Active => self.thumb_original(&sid),
            State::Trashed => match self.quarantine.locate(id) {
                Some(p) => {
                    self.check_local(&p)?;
                    fjell_fs::thumb::make_thumb(&os_path(&p)).map_err(thumb::map_error)
                }
                None => self.thumb_original(&sid),
            },
            State::Purged => Err(not_found()),
        }
    }

    fn thumb_original(&self, sid: &SourceId) -> Result<Thumb, ErrorDto> {
        let p = resolve(&self.library, sid).map_err(thumb::map_error)?;
        self.check_local(&p)?;
        self.source.thumbnail(sid).map_err(thumb::map_error)
    }

    /// `not_found`/`cloud_placeholder` fra metadata alene.
    fn check_local(&self, p: &Path) -> Result<(), ErrorDto> {
        let md = std::fs::metadata(os_path(p)).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => not_found(),
            _ => ErrorDto::new("unavailable", e.to_string()),
        })?;
        if (self.probe)(p, &md) {
            Ok(())
        } else {
            Err(ErrorDto::new("cloud_placeholder", ""))
        }
    }

    /// Gjenoppretter `ids` (bare de som ligger i «Nylig slettet» telles). Indeksen først, så flyttes
    /// filene tilbake fra karantenen. Feil ved flytting lar filen ligge; `reconcile` retter det ved oppstart.
    pub fn restore(&self, ids: &[u64]) -> Result<RestoreDto, ErrorDto> {
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
        self.restore_undone();
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
        let ids = self.trashed_ids();
        let trashed = self.fjell.sources_for(&ids);
        let _ = self.quarantine.reconcile(&trashed);
    }

    /// Etter vellykket trash: flytt filene til karantene (batch-id fra kjernen). Feil (annet volum,
    /// frakoblet) lar filen ligge: den er da «Merket, ikke flyttet» og originalen er urørt.
    pub fn stage_trashed(&self, batch: BatchId, assets: &[AssetId]) {
        let items = self.fjell.sources_for(assets);
        let _ = self.quarantine.stage(batch, &items);
    }

    /// Etter angre: alt i karantene som ikke lenger er `trashed` flyttes tilbake.
    pub fn restore_undone(&self) {
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
            let _ = self.quarantine.unstage(&back);
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
