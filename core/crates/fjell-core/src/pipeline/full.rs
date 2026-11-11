//! Full fase (PLAN-KJERNE K7 steg 3, og tidlig klynging fra steg 4). Del av `pipeline` (L3).
//!
//! Avhenger av: `hash` (`perceptual`, `low_info`), `quality` (`blur_score`), `screenshot` (`signals`),
//! `store`, `super::Ctx`. Grensesnitt: `run(cx, report)`.
//! Rayon-poolen henter `thumbnail(id)` og regner; resultatene sendes over en begrenset
//! `sync_channel` (`2 × tråder`) til én skrivetråd som skriver 100 rader per transaksjon og kaller
//! `refresh_groups` for hver 2 000. bilde (tidlig visning). `catch_unwind` per bilde; `Unavailable`
//! hoppes over uten feiltelling, øvrige feil øker `fail_count` (3 feil = hoppet over til
//! algoritmeversjonen øker). Ved avbrudd/frist slutter arbeiderne mellom bilder og skriveren committer
//! det som er mottatt. Ingen bildedata lagres (I16): miniatyren droppes etter regning.

use super::{Ctx, FailCause, Failure, PipelineError, ScanReport, StoreHandle, LOW_INFO_BIT};
use crate::hash::{low_info, perceptual};
use crate::model::{AlgoVer, AssetId, AssetSource, SourceError, Thumb};
use crate::quality::blur_score;
use crate::screenshot::signals;
use crate::store::{FingerprintRow, WorkItem};
use rayon::prelude::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{sync_channel, Receiver};

const WRITE_BATCH: usize = 100;
const EARLY_VIEW_EVERY: usize = 2_000;

enum Msg {
    Done(FingerprintRow),
    Unavailable,
    Fail(AssetId, FailCause),
}

fn valid(t: &Thumb) -> bool {
    t.w > 0 && t.h > 0 && t.data.len() == t.w as usize * t.h as usize * t.fmt.bytes_per_pixel()
}

fn analyze(src: &dyn AssetSource, item: &WorkItem, algo: AlgoVer) -> Msg {
    let id = item.id;
    let r = catch_unwind(AssertUnwindSafe(|| {
        src.thumbnail(&item.meta.source_id).map(|t| {
            if !valid(&t) {
                return None;
            }
            let (d, p) = perceptual(&t);
            let mut sig = signals(&item.meta, &t).0 & 0x0F;
            if low_info(&t) {
                sig |= LOW_INFO_BIT;
            }
            Some(FingerprintRow {
                id,
                dhash: d.0,
                phash: p.0,
                blur: blur_score(&t),
                signals: sig,
                algo,
            })
        })
    }));
    match r {
        Err(_) => Msg::Fail(id, FailCause::Panic),
        Ok(Ok(Some(row))) => Msg::Done(row),
        Ok(Ok(None)) | Ok(Err(SourceError::Corrupt)) => Msg::Fail(id, FailCause::Corrupt),
        Ok(Err(SourceError::Unavailable)) => Msg::Unavailable,
        Ok(Err(SourceError::NotFound)) => Msg::Fail(id, FailCause::NotFound),
        Ok(Err(SourceError::Failed(m))) => Msg::Fail(id, FailCause::Other(m)),
    }
}

/// Skrivetråden: tar imot til kanalen lukkes, skriver i batcher og rapporterer.
fn writer<H: StoreHandle>(
    cx: &Ctx<H>,
    rx: Receiver<Msg>,
    report: &mut ScanReport,
) -> Result<(), PipelineError> {
    let (mut rows, mut fails): (Vec<FingerprintRow>, Vec<AssetId>) = (Vec::new(), Vec::new());
    let mut since_view = 0usize;
    let flush = |rows: &mut Vec<FingerprintRow>, fails: &mut Vec<AssetId>| {
        cx.store.with_store(|s| {
            s.put_fingerprints(rows)?;
            s.record_failures(fails)
        })?;
        rows.clear();
        fails.clear();
        Ok::<_, PipelineError>(())
    };
    for msg in rx {
        match msg {
            Msg::Done(r) => {
                rows.push(r);
                report.analyzed += 1;
                since_view += 1;
            }
            Msg::Unavailable => report.skipped_unavailable += 1,
            Msg::Fail(id, cause) => {
                fails.push(id);
                report.failures.push(Failure { id, cause });
            }
        }
        cx.progress.advance(1);
        if rows.len() + fails.len() >= WRITE_BATCH {
            flush(&mut rows, &mut fails)?;
        }
        if since_view >= EARLY_VIEW_EVERY {
            flush(&mut rows, &mut fails)?;
            super::groups::refresh_groups(cx.store, cx.clock)?;
            since_view = 0;
        }
    }
    flush(&mut rows, &mut fails)
}

pub(super) fn run<H: StoreHandle>(
    cx: &Ctx<H>,
    report: &mut ScanReport,
) -> Result<(), PipelineError> {
    let work = cx.store.with_store(|s| s.work_set(cx.algo))?;
    if work.is_empty() {
        return Ok(());
    }
    let threads = cx.threads();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| PipelineError::Pool(e.to_string()))?;
    let (tx, rx) = sync_channel::<Msg>(2 * threads);
    std::thread::scope(|sc| {
        let w = sc.spawn(|| {
            let mut local = ScanReport::default();
            let r = writer(cx, rx, &mut local);
            (r, local)
        });
        pool.install(|| {
            work.par_iter().for_each(|item| {
                if cx.stop() {
                    return;
                }
                // Mislykket send = skriveren har stoppet (feil); da er det ingenting å gjøre.
                let _ = tx.send(analyze(cx.src, item, cx.algo));
            });
        });
        drop(tx);
        let (r, local) = w
            .join()
            .map_err(|_| PipelineError::Pool("skrivetråden panikket".into()))?;
        report.analyzed += local.analyzed;
        report.skipped_unavailable += local.skipped_unavailable;
        report.failures.extend(local.failures);
        r
    })
}
