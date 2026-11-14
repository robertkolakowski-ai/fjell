//! Evaluering av regelvilkår mot indeksen: ett lesepass for alle regler (`Store::rule_scan`).
//!
//! Avhenger av: model, store. Leser bare; skriver ingenting.
//! Grensesnitt: `scan(store, &[&RuleSpec], now, guard) -> Vec<Vec<Hit>>` (treff per spesifikasjon, stigende id).
//!
//! Semantikk: alle vilkår må gjelde (OG); en tom spesifikasjon treffer ingenting. Bare aktive, ubeskyttede
//! bilder er kandidater (`rule_scan` utelukker resten). «Eldre enn n dager» er strengt (`created_at` før
//! `now - n * 86400`); MIME sammenlignes uavhengig av store/små bokstaver (ASCII); mappe er `source_id`-prefiks.

use crate::model::{AssetId, RuleCondition, RuleSpec};
use crate::store::{Store, StoreError};

const DAY_SECS: i64 = 86_400;

/// Ett treff: bilde og størrelse (til forhåndsvisningens «frigjør opptil»).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub asset: AssetId,
    pub bytes: u64,
}

/// Vilkår ferdig forberedt for passet (tidsgrense regnet ut, tall klemt til lagringens bredde).
enum Test<'a> {
    CreatedBefore(i64),
    BytesOver(u64),
    BytesUnder(u64),
    Mime(&'a str),
    Folder(&'a str),
    Album(u64),
    Screenshot,
    Blurry,
    ExactDup,
}

fn compile(spec: &RuleSpec, now: i64) -> Vec<Test<'_>> {
    spec.all_of
        .iter()
        .map(|c| match c {
            RuleCondition::OlderThanDays(d) => {
                Test::CreatedBefore(now.saturating_sub((*d as i64).saturating_mul(DAY_SECS)))
            }
            RuleCondition::LargerThanBytes(n) => Test::BytesOver(*n),
            RuleCondition::SmallerThanBytes(n) => Test::BytesUnder(*n),
            RuleCondition::Mime(m) => Test::Mime(m),
            RuleCondition::InFolder(p) => Test::Folder(p),
            RuleCondition::InAlbum(a) => Test::Album(*a),
            RuleCondition::IsScreenshot => Test::Screenshot,
            RuleCondition::IsBlurry => Test::Blurry,
            RuleCondition::IsExactDuplicate => Test::ExactDup,
        })
        .collect()
}

/// Treff per spesifikasjon (samme rekkefølge som `specs`). Tomme spesifikasjoner gir tom liste.
/// Stier `guard` sperrer (keeper-reglenes beskyttede mapper) gir aldri treff.
pub fn scan(
    store: &Store,
    specs: &[&RuleSpec],
    now: i64,
    guard: super::Guard,
) -> Result<Vec<Vec<Hit>>, StoreError> {
    let compiled: Vec<Vec<Test>> = specs
        .iter()
        .map(|s| {
            if s.is_empty() {
                Vec::new()
            } else {
                compile(s, now)
            }
        })
        .collect();
    let mut out: Vec<Vec<Hit>> = vec![Vec::new(); specs.len()];
    store.rule_scan(specs, guard.is_some(), |row| {
        if guard.is_some_and(|g| g(row.source_id)) {
            return;
        }
        for (tests, hits) in compiled.iter().zip(out.iter_mut()) {
            // Tom testliste = tom regel: treffer aldri.
            if tests.is_empty() {
                continue;
            }
            let ok = tests.iter().all(|t| match t {
                Test::CreatedBefore(limit) => row.created_at < *limit,
                Test::BytesOver(n) => row.bytes > *n,
                Test::BytesUnder(n) => row.bytes < *n,
                Test::Mime(m) => row.mime.eq_ignore_ascii_case(m),
                Test::Folder(p) => row.source_id.starts_with(p),
                Test::Album(a) => row.albums.contains(a),
                Test::Screenshot => row.screenshot,
                Test::Blurry => row.blurry,
                Test::ExactDup => row.exact_dup,
            });
            if ok {
                hits.push(Hit {
                    asset: row.id,
                    bytes: row.bytes,
                });
            }
        }
    })?;
    Ok(out)
}
