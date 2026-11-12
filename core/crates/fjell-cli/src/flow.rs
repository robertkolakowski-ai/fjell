//! Fullflyt mot ekte kjerne (`Fjell`) med filsystemadapteren: skann, trash, angre, purge.
//!
//! Grensesnitt: `index`, `trash`, `undo`, `purge` (alle skriver til en `Write`) og `FlowError`.
//! Tilstanden ligger i en indeksfil (`db`) utenfor kildemappen. Kildemappen leses bare:
//! trash og angre endrer kun indeksen. `purge` krever `--fs-temp` (sletter bare i en midlertidig
//! KOPI av mappen, aldri i originalen) og eksplisitt bekreftelse (`yes`); uten bekreftelse
//! kalles verken `confirm_purge` eller noen sletting.

use crate::fsource::FsSource;
use fjell_core::api::{ApiError, Decision, Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{
    AssetId, Clock, ConsequenceCode, DeleteOutcome, PlatformDelete, PurgeTicket, SourceId,
    SuggestionKind,
};
use fjell_core::store::Store;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Feil fra kommandoene. `NeedsConfirm` betyr at ingenting ble gjort (exit-kode 3).
#[derive(Debug)]
pub enum FlowError {
    Usage(String),
    NeedsConfirm(String),
    Failed(String),
}

impl std::fmt::Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlowError::Usage(s) | FlowError::NeedsConfirm(s) | FlowError::Failed(s) => {
                f.write_str(s)
            }
        }
    }
}

fn fail(e: impl std::fmt::Debug) -> FlowError {
    FlowError::Failed(format!("{e:?}"))
}

fn w(out: &mut dyn Write, s: String) -> Result<(), FlowError> {
    writeln!(out, "{s}").map_err(fail)
}

struct SysClock;
impl Clock for SysClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64)
    }
}

fn open(dir: &Path, db: &Path) -> Result<Fjell, FlowError> {
    if db.starts_with(dir) {
        return Err(FlowError::Usage(
            "indeksfilen må ligge utenfor kildemappen".into(),
        ));
    }
    let store = Store::open(db).map_err(fail)?;
    Ok(Fjell::new(
        store,
        Arc::new(FsSource::new(dir).with_index(db)),
        Arc::new(SysClock),
    ))
}

fn dup_cards(f: &Fjell) -> Result<Vec<fjell_core::api::Card>, FlowError> {
    Ok(f.cards(SuggestionKind::ExactDup, 10_000, None)
        .map_err(fail)?
        .cards)
}

/// Skanner mappen inn i indeksen og skriver antall eksakte duplikatkort.
pub fn index(dir: &Path, db: &Path, out: &mut dyn Write) -> Result<(), FlowError> {
    let f = open(dir, db)?;
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .map_err(fail)?;
    let r = f
        .wait_scan()
        .ok_or_else(|| fail("ingen rapport"))?
        .map_err(fail)?;
    if !r.failures.is_empty() {
        return Err(fail(&r.failures));
    }
    w(
        out,
        format!(
            "scanned new={} changed={} removed={}",
            r.new, r.changed, r.removed
        ),
    )?;
    let cards = dup_cards(&f)?;
    let bytes: u64 = cards.iter().map(|c| c.bytes).sum();
    w(out, format!("exact_dup: {} ({bytes} bytes)", cards.len()))
}

/// Legger alle ventende eksakte-duplikatkort i «Nylig slettet» (bare indeks) som én batch.
pub fn trash(dir: &Path, db: &Path, yes: bool, out: &mut dyn Write) -> Result<(), FlowError> {
    let f = open(dir, db)?;
    let cards = dup_cards(&f)?;
    if cards.is_empty() {
        return w(out, "nothing to trash".into());
    }
    let ds: Vec<_> = cards.iter().map(|c| (c.id, Decision::Trash)).collect();
    let batch = match f.decide_many(ds.clone()) {
        Err(ApiError::AckRequired(codes)) if yes => {
            f.decide_many_confirmed(ds, codes).map_err(fail)?
        }
        Err(ApiError::AckRequired(codes)) => {
            return Err(FlowError::NeedsConfirm(format!(
                "krever bekreftelse (--yes): {codes:?}"
            )))
        }
        r => r.map_err(fail)?,
    };
    for c in &cards {
        w(
            out,
            format!("trashed asset={} bytes={}", c.asset.0, c.bytes),
        )?;
    }
    w(out, format!("batch={}", batch.0))
}

/// Angrer siste batch.
pub fn undo(dir: &Path, db: &Path, out: &mut dyn Write) -> Result<(), FlowError> {
    let r = open(dir, db)?.undo().map_err(fail)?;
    w(
        out,
        format!("undone batch={} restored={}", r.batch.0, r.restored),
    )
}

/// Sletter i en midlertidig kopi via plattformsdobbelen. Avviser stier som kan forlate kopien.
struct TempCopyDelete(PathBuf);
impl PlatformDelete for TempCopyDelete {
    fn delete(
        &self,
        _t: &PurgeTicket,
        ids: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, DeleteOutcome)> {
        ids.iter()
            .map(|(a, s)| {
                let safe =
                    !s.0.starts_with('/') && s.0.split('/').all(|c| !c.is_empty() && c != "..");
                let ok = safe && fs::remove_file(self.0.join(&s.0)).is_ok();
                let o = if ok {
                    DeleteOutcome::Deleted
                } else {
                    DeleteOutcome::Failed
                };
                (*a, o)
            })
            .collect()
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let p = e?.path();
        let t = to.join(p.file_name().unwrap_or_default());
        if p.is_dir() {
            copy_tree(&p, &t)?;
        } else {
            fs::copy(&p, &t)?;
        }
    }
    Ok(())
}

/// Purge av `ids` (må være i «Nylig slettet»). Krever `fs_temp` og `yes`; sletter bare i en kopi.
/// Skriver stien til kopien slik at kalleren kan rydde den.
pub fn purge(
    dir: &Path,
    db: &Path,
    ids: &[u64],
    fs_temp: bool,
    yes: bool,
    out: &mut dyn Write,
) -> Result<(), FlowError> {
    if !fs_temp {
        return Err(FlowError::Usage(
            "purge støttes bare med --fs-temp (sletting i midlertidig kopi)".into(),
        ));
    }
    let f = open(dir, db)?;
    let intent = f
        .begin_purge(ids.iter().map(|i| AssetId(*i)).collect())
        .map_err(fail)?;
    w(
        out,
        format!("purge: {} files, {} bytes", intent.ids.len(), intent.bytes),
    )?;
    if !yes {
        return Err(FlowError::NeedsConfirm(
            "purge krever bekreftelse (--yes); ingenting slettet".into(),
        ));
    }
    let ticket = match f.confirm_purge(&intent) {
        Err(ApiError::AckRequired(codes)) => f
            .confirm_purge_confirmed(&intent, codes as Vec<ConsequenceCode>)
            .map_err(fail)?,
        r => r.map_err(fail)?,
    };
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let copy = std::env::temp_dir().join(format!("fjell-purge-{}-{nanos}", std::process::id()));
    copy_tree(dir, &copy).map_err(fail)?;
    let results = TempCopyDelete(copy.clone()).delete(&ticket, &f.sources_for(ticket.ids()));
    let rep = f.finish_purge(ticket, results).map_err(fail)?;
    w(
        out,
        format!(
            "purged={} kept_trashed={}",
            rep.purged.len(),
            rep.kept_trashed.len()
        ),
    )?;
    w(out, format!("temp_copy={}", copy.display()))
}
