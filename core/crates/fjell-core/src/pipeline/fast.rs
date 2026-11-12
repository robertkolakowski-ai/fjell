//! Rask fase (PLAN-KJERNE K7 steg 2). Del av `pipeline` (L3).
//!
//! Avhenger av: `hash` (`needs_hash`, `content_hash`), `store`, `super::Ctx`. Grensesnitt:
//! `plan_totals(cx)` (antall å hashe og antall i arbeidssettet) og `run(cx, report)`.
//! Bare filer som deler størrelse med en annen leses (størrelsesforfilter). BLAKE3 kjøres parallelt
//! i biter på 128 filer; resultatet skrives per bit, så avbrudd taper høyst én bit. `Unavailable` hoppes
//! over uten feiltelling og forsøkes igjen neste skanning (hashen står tom). Ingen dekoding.

use super::{Ctx, FailCause, Failure, PipelineError, ScanReport, StoreHandle};
use crate::hash::{content_hash, needs_hash, HashCandidate};
use crate::model::{AssetId, SourceError, SourceId};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

const CHUNK: usize = 128;

fn candidates<H: StoreHandle>(cx: &Ctx<H>) -> Result<Vec<(AssetId, SourceId)>, PipelineError> {
    let rows = cx.store.with_store(|s| s.hash_rows())?;
    let cands: Vec<HashCandidate> = rows
        .iter()
        .map(|r| HashCandidate {
            id: r.id,
            source: r.source.clone(),
            bytes: r.bytes,
            hashed: r.hashed,
        })
        .collect();
    let src: BTreeMap<AssetId, SourceId> = rows.into_iter().map(|r| (r.id, r.source)).collect();
    Ok(needs_hash(&cands)
        .into_iter()
        .map(|id| (id, src[&id].clone()))
        .collect())
}

pub(super) fn plan_totals<H: StoreHandle>(cx: &Ctx<H>) -> Result<(u64, u64), PipelineError> {
    let hash = candidates(cx)?.len() as u64;
    let work = cx.store.with_store(|s| s.work_set(cx.algo))?.len() as u64;
    Ok((hash, work))
}

enum Hashed {
    Ok([u8; 32]),
    Unavailable,
    Fail(FailCause),
}

fn hash_one(src: &dyn crate::model::AssetSource, id: &SourceId) -> Hashed {
    let r = catch_unwind(AssertUnwindSafe(|| {
        src.read_bytes(id)
            .and_then(|mut r| content_hash(&mut r).map_err(|e| SourceError::Failed(e.to_string())))
    }));
    match r {
        Err(_) => Hashed::Fail(FailCause::Panic),
        Ok(Ok(h)) => Hashed::Ok(h),
        Ok(Err(SourceError::Unavailable)) => Hashed::Unavailable,
        Ok(Err(SourceError::Corrupt)) => Hashed::Fail(FailCause::Corrupt),
        Ok(Err(SourceError::NotFound)) => Hashed::Fail(FailCause::NotFound),
        Ok(Err(SourceError::Failed(m))) => Hashed::Fail(FailCause::Other(m)),
    }
}

pub(super) fn run<H: StoreHandle>(
    cx: &Ctx<H>,
    report: &mut ScanReport,
) -> Result<(), PipelineError> {
    let todo = candidates(cx)?;
    if todo.is_empty() {
        return Ok(());
    }
    cx.progress.set_phase(super::Phase::Exact);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(cx.threads())
        .build()
        .map_err(|e| PipelineError::Pool(e.to_string()))?;
    for chunk in todo.chunks(CHUNK) {
        if cx.stop() {
            break;
        }
        let results: Vec<(AssetId, Option<Hashed>)> = pool.install(|| {
            chunk
                .par_iter()
                .map(|(id, src)| {
                    // Avbrutt midt i en bit: filen hoppes over og tas neste gang.
                    if cx.stop() {
                        return (*id, None);
                    }
                    (*id, Some(hash_one(cx.src, src)))
                })
                .collect()
        });
        let mut ok = Vec::new();
        for (id, r) in results {
            match r {
                None => {}
                Some(Hashed::Ok(h)) => ok.push((id, h)),
                // `Unavailable` er ikke en feil: filen hashes igjen når den er lastet ned.
                Some(Hashed::Unavailable) => report.skipped_unavailable += 1,
                Some(Hashed::Fail(cause)) => report.failures.push(Failure { id, cause }),
            }
        }
        report.hashed += ok.len() as u64;
        cx.store.with_store(|s| s.put_content_hashes(&ok))?;
        cx.progress.advance(chunk.len() as u64);
    }
    Ok(())
}
