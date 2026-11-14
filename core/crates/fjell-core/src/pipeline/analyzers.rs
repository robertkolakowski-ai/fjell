//! Valgfrie analysatorer etter skanning (V1): øyne til serier, ansikter til personer. Del av `pipeline` (L3).
//!
//! Avhenger av: people (`cluster_new_faces`), store, model (`EyeAnalyzer`, `FaceAnalyzer`,
//! `AssetSource`, `Clock`), `super::groups`. Grensesnitt:
//! - `run_analyzers(src, store, eye, face, clock, cancel) -> AnalyzerReport`: øyne for aktive bilder uten
//!   øyeanalyse, så (bare når `face_index` er på) ansikter og klynging, og til slutt `refresh_groups` slik at
//!   seriene bruker de nye øyeresultatene.
//!
//! Regler (CLAUDE.md og V1-PLAN):
//! - Ansiktsindeksen er av som standard: er den av, kalles `FaceAnalyzer` aldri og ingen miniatyr hentes for ansikter.
//! - `AnalyzeError::Unavailable` (Null-analysatorene, ingen modell) avbryter steget uten å lagre noe: aldri
//!   «ingen lukkede øyne» eller «ingen ansikter» for noe som ikke er analysert.
//! - Miniatyrer hentes uten lås på lagringen og droppes etter analysen; bare avledede tall lagres (I16).
//! - Ingenting her sletter eller endrer `asset.state`; avbrudd (`cancel`) stopper mellom bilder.
//!
//! Kjent begrensning: bilder uten ansikter har ingen markør og analyseres på nytt ved neste kjøring
//! (markør krever en delt schema-endring, meldt i docs/v1/V1-PLAN.md).

use super::{groups::refresh_groups, PipelineError, StoreHandle};
use crate::model::{
    AnalyzeError, AssetId, AssetSource, Clock, EyeAnalyzer, EyeReport, FaceAnalyzer, Kind,
    SourceId, Thumb,
};
use crate::people::{self, PeopleParams};
use crate::store::StoreError;
use std::sync::atomic::{AtomicBool, Ordering};

/// Algoritmeversjon for øyeanalyse (økes når modellen byttes, slik at bilder analyseres på nytt).
pub const EYE_ALGO: u32 = 1;
/// Algoritmeversjon for ansiktsanalyse.
pub const FACE_ALGO: u32 = 1;

/// Hva analysatorene gjorde.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalyzerReport {
    /// Bilder med nytt øyeresultat.
    pub eyes_analyzed: u64,
    /// Nye ansikter lagret.
    pub faces_added: u64,
    /// Ansikter tildelt en person.
    pub faces_assigned: u64,
    /// Nye personer opprettet.
    pub new_people: u64,
    /// En analysator svarte `Unavailable` og steget ble hoppet over.
    pub unavailable: bool,
}

fn sources<H: StoreHandle>(
    store: &H,
    ids: Vec<AssetId>,
) -> Result<Vec<(AssetId, SourceId)>, PipelineError> {
    Ok(store.with_store(|s| s.source_ids(&ids))?)
}

fn thumb(src: &dyn AssetSource, id: &SourceId) -> Option<Thumb> {
    src.thumbnail(id).ok()
}

/// Kjører øyne og (om på) ansikter. Se modulhodet.
pub fn run_analyzers<H: StoreHandle>(
    src: &dyn AssetSource,
    store: &H,
    eye: &dyn EyeAnalyzer,
    face: &dyn FaceAnalyzer,
    clock: &dyn Clock,
    cancel: &AtomicBool,
) -> Result<AnalyzerReport, PipelineError> {
    let mut rep = AnalyzerReport::default();
    // Øyne: bare bilder uten resultat for denne versjonen.
    let missing = store.with_store(|s| s.eye_missing(EYE_ALGO))?;
    let mut rows: Vec<(AssetId, EyeReport)> = Vec::new();
    for (id, sid) in sources(store, missing)? {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let Some(t) = thumb(src, &sid) else { continue };
        match eye.analyze(&t) {
            Ok(r) => rows.push((id, r)),
            Err(AnalyzeError::Unavailable) => {
                rep.unavailable = true;
                break;
            }
            Err(AnalyzeError::Failed(_)) => {}
        }
    }
    rep.eyes_analyzed = rows.len() as u64;
    if !rows.is_empty() {
        store.with_store(|s| s.put_eye_results(&rows, EYE_ALGO))?;
    }
    // Ansikter: ingenting skjer med indeksen av.
    if store.with_store(|s| s.face_index_enabled())? {
        let ids: Vec<AssetId> = store.with_store(|s| {
            s.index_rows().map(|r| {
                r.into_iter()
                    .filter(|r| r.kind == Kind::Photo)
                    .map(|r| r.id)
                    .collect()
            })
        })?;
        for (id, sid) in sources(store, ids)? {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            if store.with_store(|s| s.asset_has_faces(id, FACE_ALGO))? {
                continue;
            }
            let Some(t) = thumb(src, &sid) else { continue };
            // Analysen går utenfor låsen; indeksen kan ha blitt slått av eller slettet i mellomtiden.
            match face.detect(&t) {
                Ok(faces) if !faces.is_empty() => {
                    let n = store.with_store(|s| -> Result<usize, StoreError> {
                        if s.face_index_enabled()? {
                            Ok(s.put_faces(id, &faces, FACE_ALGO)?.len())
                        } else {
                            Ok(0)
                        }
                    })?;
                    rep.faces_added += n as u64;
                }
                Ok(_) => {}
                Err(AnalyzeError::Unavailable) => {
                    rep.unavailable = true;
                    break;
                }
                Err(AnalyzeError::Failed(_)) => {}
            }
        }
        let c = store
            .with_store(|s| people::cluster_new_faces(s, &PeopleParams::default(), clock.now()))?;
        rep.faces_assigned = c.assigned;
        rep.new_people = c.new_people;
    }
    if rep.eyes_analyzed > 0 {
        refresh_groups(store, clock)?;
    }
    Ok(rep)
}
