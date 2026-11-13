//! Karantene (ARKITEKTUR beslutning 20): `stage` flytter fil til `<volum>/.fjell-karantene/<batch>/`
//! med manifest, `unstage` flytter tilbake, `reconcile` avstemmer ved oppstart. Flytter, sletter aldri.
//!
//! Grensesnitt: trait `Quarantine`, `FsQuarantine` (rename på samme volum, aldri kopiering),
//! `QuarantineError`, `Reconciled`. Hjelpere for sletting: `locate`, `forget`, `staged`.
//! Regler:
//! - Innholdet i originalfiler endres aldri; bare stien byttes (rename).
//! - Frakoblet rot gir `Unavailable` for alle, og avstemming endrer ingenting (T27).
//! - Annet volum (`CrossesDevices`) gir `CrossVolume`: filen blir liggende («Merket, ikke flyttet»).
//! - Kollisjon ved gjenoppretting gir `navn (gjenopprettet).ext`, aldri overskriving.
//!
//! Åpent punkt: Windows Papirkurv husker karantenestien som opprinnelse for filer som tømmes
//! derfra (ARKITEKTUR del 9: karantene og skyspeilede mapper).

use crate::manifest::{scan_batches, Entry, Manifest, README_FILE, README_TEXT};
use crate::path::{os_path, resolve};
use fjell_core::model::{AssetId, BatchId, SourceId};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const QUARANTINE_DIR: &str = ".fjell-karantene";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuarantineError {
    /// Roten/volumet er frakoblet. Ikke det samme som at filen er borte.
    Unavailable,
    NotFound,
    /// Karantenen ligger på et annet volum enn filen; ingen kopiering.
    CrossVolume,
    Failed(String),
}

/// Utfall av oppstartsavstemming per id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reconciled {
    /// Fil i karantene og `trashed` i indeksen: i samsvar.
    StillStaged,
    /// Fil i karantene men ikke `trashed`: flyttet tilbake til oppgitt sti.
    Restored(SourceId),
    /// `trashed`, men filen ligger på opprinnelig plass (flytting aldri gjort eller ikke mulig).
    NotMoved,
    /// `trashed`, men verken i karantene eller på opprinnelig plass: indeks settes `purged` (`cause=external`).
    Missing,
    /// Fil ligger i karantene uten manifestoppføring (manifest tapt). Ikke tapt og ikke `purged`;
    /// kan ikke gjenopprettes automatisk fordi opprinnelig sti er ukjent.
    Orphaned,
    /// Roten er frakoblet: ingenting er endret, ingenting skal markeres borte.
    Unavailable,
}

pub trait Quarantine: Send + Sync {
    fn stage(
        &self,
        batch: BatchId,
        items: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, Result<(), QuarantineError>)>;
    fn unstage(&self, ids: &[AssetId]) -> Vec<(AssetId, Result<SourceId, QuarantineError>)>;
    fn reconcile(&self, trashed: &[(AssetId, SourceId)]) -> Vec<(AssetId, Reconciled)>;
}

/// Karantene for én bibliotekrot. `base` er volumroten karantenemappen ligger i (på Windows
/// `C:\` eller `\\server\share\`); for portabel bruk er den lik bibliotekroten.
pub struct FsQuarantine {
    library: PathBuf,
    dir: PathBuf,
    /// Id-er som er gjenopprettet (`unstage`) i denne prosessen og ikke staged på nytt siden.
    /// «Tøm» skal aldri falle tilbake til opprinnelig sti for dem (F2).
    restored: std::sync::Mutex<std::collections::HashSet<u64>>,
}

fn map_io(e: std::io::Error) -> QuarantineError {
    match e.kind() {
        ErrorKind::NotFound => QuarantineError::NotFound,
        ErrorKind::CrossesDevices => QuarantineError::CrossVolume,
        _ => QuarantineError::Failed(e.to_string()),
    }
}

fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

impl FsQuarantine {
    pub fn new(library: impl Into<PathBuf>, base: impl AsRef<Path>) -> FsQuarantine {
        FsQuarantine {
            library: library.into(),
            dir: base.as_ref().join(QUARANTINE_DIR),
            restored: Default::default(),
        }
    }

    /// Er id gjenopprettet fra karantenen og ikke staged på nytt? Da ligger filen aktiv på
    /// opprinnelig sti og må ikke slettes av «Tøm».
    pub fn was_restored(&self, id: AssetId) -> bool {
        self.restored.lock().is_ok_and(|r| r.contains(&id.0))
    }

    fn mark_restored(&self, ids: &[u64], on: bool) {
        if let Ok(mut r) = self.restored.lock() {
            for i in ids {
                if on {
                    r.insert(*i);
                } else {
                    r.remove(i);
                }
            }
        }
    }

    /// Karantene i bibliotekroten (portabelt oppsett og tester).
    pub fn in_library(library: impl Into<PathBuf>) -> FsQuarantine {
        let library = library.into();
        FsQuarantine::new(library.clone(), &library)
    }

    fn offline(&self) -> bool {
        !os_path(&self.library).is_dir()
    }

    fn batch_dir(&self, batch: BatchId) -> PathBuf {
        self.dir.join(batch.0.to_string())
    }

    /// Hvor ligger den staged filen akkurat nå (for «Tøm»)? `None` hvis id ikke er i karantene.
    pub fn locate(&self, id: AssetId) -> Option<PathBuf> {
        scan_batches(&os_path(&self.dir))
            .into_iter()
            .find_map(|(d, m)| {
                m.entries
                    .iter()
                    .find(|e| e.id == id.0)
                    .map(|e| d.join(&e.stored))
            })
    }

    /// Alle staged id-er med opprinnelig sti (skanning regner dem som tilstedeværende).
    pub fn staged(&self) -> Vec<(AssetId, SourceId)> {
        scan_batches(&os_path(&self.dir))
            .into_iter()
            .flat_map(|(_, m)| m.entries)
            .map(|e| (AssetId(e.id), SourceId(e.original)))
            .collect()
    }

    /// Fjerner manifestoppføringen etter at filen er slettet utenfra (Papirkurv/permanent).
    pub fn forget(&self, id: AssetId) {
        self.drop_entries(&[id.0]);
    }

    /// Fjern oppføringer; slett manifest og mappe (bare tom mappe + README) når siste er borte.
    fn drop_entries(&self, ids: &[u64]) {
        for (d, mut m) in scan_batches(&os_path(&self.dir)) {
            let before = m.entries.len();
            m.entries.retain(|e| !ids.contains(&e.id));
            if m.entries.len() == before {
                continue;
            }
            if m.entries.is_empty() {
                let _ = fs::remove_file(d.join(crate::manifest::MANIFEST_FILE));
                let _ = fs::remove_file(d.join(README_FILE));
                let _ = fs::remove_dir(&d); // feiler (og beholdes) hvis noe annet ligger der
            } else {
                let _ = m.save(&d);
            }
        }
    }

    /// Finnes en fil `<id>[.ext]` i en batchmappe uten manifestoppføring?
    fn has_orphan(&self, id: AssetId) -> bool {
        let Ok(batches) = fs::read_dir(os_path(&self.dir)) else {
            return false;
        };
        batches.filter_map(|b| b.ok()).any(|b| {
            fs::read_dir(b.path()).is_ok_and(|rd| {
                rd.filter_map(|f| f.ok()).any(|f| {
                    let name = f.file_name().to_string_lossy().into_owned();
                    split_name(&name).0 == id.0.to_string()
                })
            })
        })
    }

    fn restore_target(&self, original: &SourceId) -> Result<PathBuf, QuarantineError> {
        let wanted = resolve(&self.library, original).map_err(|_| QuarantineError::NotFound)?;
        if !os_path(&wanted).exists() {
            return Ok(wanted);
        }
        let name = wanted
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("fil")
            .to_string();
        let (stem, ext) = split_name(&name);
        for n in 1u32.. {
            let suffix = if n == 1 {
                "gjenopprettet".to_string()
            } else {
                format!("gjenopprettet {n}")
            };
            let cand = wanted.with_file_name(format!("{stem} ({suffix}){ext}"));
            if !os_path(&cand).exists() {
                return Ok(cand);
            }
        }
        unreachable!()
    }

    fn move_back(&self, from: &Path, e: &Entry) -> Result<SourceId, QuarantineError> {
        let target = self.restore_target(&SourceId(e.original.clone()))?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(os_path(parent)).map_err(map_io)?;
        }
        fs::rename(os_path(from), os_path(&target)).map_err(map_io)?;
        let rel = target.strip_prefix(&self.library).unwrap_or(&target);
        let parts: Vec<_> = rel.iter().map(|s| s.to_string_lossy()).collect();
        Ok(SourceId(parts.join("/")))
    }
}

impl Quarantine for FsQuarantine {
    fn stage(
        &self,
        batch: BatchId,
        items: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, Result<(), QuarantineError>)> {
        if self.offline() {
            return items
                .iter()
                .map(|(id, _)| (*id, Err(QuarantineError::Unavailable)))
                .collect();
        }
        let already: Vec<u64> = self.staged().iter().map(|(i, _)| i.0).collect();
        let bdir = self.batch_dir(batch);
        // Planlegg først (hensikt), skriv manifest, flytt deretter.
        let mut manifest = Manifest::load(&os_path(&bdir)).unwrap_or(Manifest {
            batch: batch.0,
            entries: Vec::new(),
        });
        let mut plan: Vec<(AssetId, PathBuf, Entry)> = Vec::new();
        let mut out: Vec<(AssetId, Result<(), QuarantineError>)> = Vec::new();
        for (id, sid) in items {
            if already.contains(&id.0) {
                out.push((*id, Ok(())));
                continue;
            }
            let src = match resolve(&self.library, sid) {
                Ok(p) => p,
                Err(_) => {
                    out.push((*id, Err(QuarantineError::NotFound)));
                    continue;
                }
            };
            let md = match fs::metadata(os_path(&src)) {
                Ok(m) if m.is_file() => m,
                Ok(_) => {
                    out.push((*id, Err(QuarantineError::NotFound)));
                    continue;
                }
                Err(e) => {
                    out.push((*id, Err(map_io(e))));
                    continue;
                }
            };
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
            let stored = if ext.is_empty() {
                id.0.to_string()
            } else {
                format!("{}.{ext}", id.0)
            };
            let entry = Entry {
                id: id.0,
                original: sid.0.clone(),
                stored,
                bytes: md.len(),
            };
            plan.push((*id, src, entry));
        }
        if plan.is_empty() {
            return out;
        }
        let planned: Vec<u64> = plan.iter().map(|(id, _, _)| id.0).collect();
        self.mark_restored(&planned, false);
        manifest
            .entries
            .extend(plan.iter().map(|(_, _, e)| e.clone()));
        let prepared = manifest
            .save(&os_path(&bdir))
            .and_then(|_| fs::write(os_path(&bdir).join(README_FILE), README_TEXT));
        if let Err(e) = prepared {
            let err = map_io(e);
            out.extend(plan.iter().map(|(id, _, _)| (*id, Err(err.clone()))));
            return out;
        }
        let mut failed: Vec<u64> = Vec::new();
        for (id, src, entry) in &plan {
            let r = fs::rename(os_path(src), os_path(&bdir).join(&entry.stored)).map_err(map_io);
            if r.is_err() {
                failed.push(id.0);
            }
            out.push((*id, r));
        }
        if !failed.is_empty() {
            self.drop_entries(&failed);
        }
        out
    }

    fn unstage(&self, ids: &[AssetId]) -> Vec<(AssetId, Result<SourceId, QuarantineError>)> {
        if self.offline() {
            return ids
                .iter()
                .map(|id| (*id, Err(QuarantineError::Unavailable)))
                .collect();
        }
        let batches = scan_batches(&os_path(&self.dir));
        let mut out = Vec::new();
        let mut done: Vec<u64> = Vec::new();
        for id in ids {
            let found = batches
                .iter()
                .find_map(|(d, m)| m.entries.iter().find(|e| e.id == id.0).map(|e| (d, e)));
            let Some((d, e)) = found else {
                out.push((*id, Err(QuarantineError::NotFound)));
                continue;
            };
            match self.move_back(&d.join(&e.stored), e) {
                Ok(sid) => {
                    done.push(id.0);
                    out.push((*id, Ok(sid)));
                }
                Err(err) => out.push((*id, Err(err))),
            }
        }
        self.mark_restored(&done, true);
        self.drop_entries(&done);
        out
    }

    fn reconcile(&self, trashed: &[(AssetId, SourceId)]) -> Vec<(AssetId, Reconciled)> {
        if self.offline() {
            return trashed
                .iter()
                .map(|(id, _)| (*id, Reconciled::Unavailable))
                .collect();
        }
        let is_trashed = |id: u64| trashed.iter().any(|(t, _)| t.0 == id);
        let mut out: Vec<(AssetId, Reconciled)> = Vec::new();
        let mut handled: Vec<u64> = Vec::new();
        let mut drop: Vec<u64> = Vec::new();
        for (d, m) in scan_batches(&os_path(&self.dir)) {
            for e in &m.entries {
                let stored = d.join(&e.stored);
                let id = AssetId(e.id);
                if os_path(&stored).exists() {
                    if is_trashed(e.id) {
                        out.push((id, Reconciled::StillStaged));
                    } else if let Ok(sid) = self.move_back(&stored, e) {
                        drop.push(e.id);
                        out.push((id, Reconciled::Restored(sid)));
                    }
                    // Feilet tilbakeflytting: oppføringen beholdes og prøves ved neste oppstart.
                    handled.push(e.id);
                } else {
                    // Avbrutt flytting: filen ligger ikke i karantene. Manifestet var bare hensikt.
                    drop.push(e.id);
                }
            }
        }
        self.drop_entries(&drop);
        for (id, sid) in trashed {
            if handled.contains(&id.0) {
                continue;
            }
            let orig = resolve(&self.library, sid).map(|p| os_path(&p).exists());
            out.push((
                *id,
                if orig.unwrap_or(false) {
                    Reconciled::NotMoved
                } else if self.has_orphan(*id) {
                    Reconciled::Orphaned
                } else {
                    Reconciled::Missing
                },
            ));
        }
        out
    }
}
