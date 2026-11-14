//! V1-flaten for `Fjell` (L4): beskyttelse, regler, personer, ansiktsindeks, dashboard, seriesammenligning,
//! innstillinger og terskler. Del av `api`; `impl FjellApi for Fjell` (i `fjell_ops`) delegerer hit.
//!
//! Avhenger av: `super::fjell` (`Fjell`, feilmapping), protect, rules, people, dashboard, series, pipeline
//! (`tuning`, `series_specs`-typene), store, model. Grensesnitt: kun `pub(super)` metoder på `Fjell`.
//!
//! Regler (CLAUDE.md og docs/v1/V1-PLAN.md):
//! - Endret beskyttelse evaluerer reglene på nytt (beskyttede bilder gir aldri regeltreff).
//! - Ingenting her sletter bilder. `unprotect`, `rule_delete` og `person_merge` er konfigurasjon og kan angres
//!   (`protect_set`, `rule_restore`, `person_unmerge`). `face_index_wipe` er det eneste som fjerner data
//!   (avledede embeddings og personer, aldri bilder) og er et eksplisitt personvernvalg i UI.
//! - Feil returneres som koder i `ApiError::Internal` (stabile snake_case-koder, aldri tekst): `empty_folder`,
//!   `unknown_target`, `empty_rule`, `empty_name`, `invalid_condition`, `name_too_long`, `rule_not_found`, `unknown_person`,
//!   `not_merged`, `unknown_series`.
//! - Alle lesninger tar indekslåsen kort; ingen miniatyrer dekodes her.

use super::fjell::{internal, Fjell};
use super::{
    ApiError, PersonView, ProtectionEntry, RuleDraft, RulePreview, RuleView, Sensitivities,
    Sensitivity, SeriesCompare, SeriesMemberView, Settings, Thresholds, FACE_INDEX_KEY,
};
use crate::dashboard::{self, Dashboard};
use crate::model::{
    AssetId, DashboardData, EyeReport, Hash64, Kind, PersonId, ProtectScope, ReasonCode,
    ReasonDetail, RuleId, RuleSpec, SeriesId, SuggestionKind,
};
use crate::people;
use crate::pipeline::{tuning, Tuning};
use crate::protect::{self, ProtectError};
use crate::quality::{BLUR_ABS_MAX, LOW_CONTRAST};
use crate::rules::{self, RuleError};
use crate::series::{rank_series, SeriesItem};
use crate::store::{CardRow, Store, StoreError};

/// V1-feilkoder som `ApiError::Internal` bærer. UI oversetter via i18n-nøkkelen `error.<kode>`.
pub const V1_ERROR_CODES: [&str; 10] = [
    "empty_folder",
    "unknown_target",
    "empty_rule",
    "empty_name",
    "name_too_long",
    "invalid_condition",
    "rule_not_found",
    "unknown_person",
    "not_merged",
    "unknown_series",
];

fn code(c: &str) -> ApiError {
    debug_assert!(V1_ERROR_CODES.contains(&c), "ukjent feilkode {c}");
    ApiError::Internal(c.to_string())
}

fn protect_err(e: ProtectError) -> ApiError {
    match e {
        ProtectError::EmptyFolder => code("empty_folder"),
        ProtectError::UnknownTarget(_) => code("unknown_target"),
        ProtectError::Store(e) => internal(e),
    }
}

fn rule_err(e: RuleError) -> ApiError {
    match e {
        RuleError::EmptySpec => code("empty_rule"),
        RuleError::EmptyName => code("empty_name"),
        RuleError::NameTooLong => code("name_too_long"),
        RuleError::InvalidCondition => code("invalid_condition"),
        RuleError::NotFound => code("rule_not_found"),
        RuleError::Store(e) => internal(e),
    }
}

/// Sensitivitet leses fra `setting`; manglende eller ukjent kode gir `Normal`.
fn read_sensitivities(store: &Store) -> Sensitivities {
    let mut s = Sensitivities::default();
    for k in SuggestionKind::ALL {
        let v = store
            .setting(&super::sensitivity_key(k))
            .ok()
            .flatten()
            .and_then(|c| Sensitivity::parse(&c));
        if let Some(v) = v {
            s.set(k, v);
        }
    }
    s
}

fn tuning_for(s: &Sensitivities) -> Tuning {
    tuning(|k| Some(s.get(k).as_str().to_string()))
}

fn item(r: &crate::store::IndexRow, eyes: Option<EyeReport>) -> Option<SeriesItem> {
    Some(SeriesItem {
        id: r.id,
        taken_at: r.created_at,
        dhash: Hash64(r.dhash?),
        phash: Hash64(r.phash?),
        blur: r.blur.unwrap_or(LOW_CONTRAST),
        eyes,
    })
}

impl Fjell {
    pub(super) fn v1_protect(&self, scope: ProtectScope, on: bool) -> Result<(), ApiError> {
        let now = self.sh.clock.now();
        let mut a = self.actions();
        let s = a.store_mut();
        if on {
            protect::set(s, &scope, now).map_err(protect_err)?;
        } else {
            protect::unset(s, &scope).map_err(protect_err)?;
        }
        // Regler gir aldri treff på beskyttede bilder: tell treffene på nytt med en gang.
        rules::evaluate(s, now).map_err(rule_err)?;
        Ok(())
    }

    pub(super) fn v1_protections(&self) -> Result<Vec<ProtectionEntry>, ApiError> {
        let list = protect::list(self.actions().store()).map_err(protect_err)?;
        Ok(list
            .into_iter()
            .map(|(scope, created_at)| ProtectionEntry { scope, created_at })
            .collect())
    }

    pub(super) fn v1_rules_list(&self) -> Result<Vec<RuleView>, ApiError> {
        let items = rules::list(self.actions().store()).map_err(rule_err)?;
        Ok(items
            .into_iter()
            .map(|r| RuleView {
                id: r.id,
                name: r.name,
                spec: r.spec,
                enabled: r.enabled,
                hits: r.hits,
            })
            .collect())
    }

    pub(super) fn v1_rule_save(&self, d: RuleDraft) -> Result<RuleId, ApiError> {
        let now = self.sh.clock.now();
        let id = rules::save(self.actions().store_mut(), d.id, &d.name, &d.spec, now)
            .map_err(rule_err)?;
        Ok(id)
    }

    pub(super) fn v1_rule_preview(&self, spec: RuleSpec) -> Result<RulePreview, ApiError> {
        let now = self.sh.clock.now();
        let p = rules::preview(self.actions().store(), &spec, now).map_err(rule_err)?;
        Ok(RulePreview {
            hits: p.hits,
            bytes: p.bytes,
            sample: p.sample,
        })
    }

    pub(super) fn v1_rule_change(&self, id: RuleId, op: RuleOp) -> Result<(), ApiError> {
        let now = self.sh.clock.now();
        let mut a = self.actions();
        let s = a.store_mut();
        match op {
            RuleOp::Enabled(on) => rules::toggle(s, id, on, now),
            RuleOp::Delete => rules::delete(s, id, now),
            RuleOp::Restore => rules::restore(s, id, now),
        }
        .map(|_| ())
        .map_err(rule_err)
    }

    pub(super) fn v1_people_list(&self) -> Result<Vec<PersonView>, ApiError> {
        let list = people::list(self.actions().store()).map_err(internal)?;
        Ok(list
            .into_iter()
            .map(|p| PersonView {
                id: p.id,
                name: p.name,
                face_count: p.face_count,
                asset_count: p.asset_count,
                cover: p.cover,
                protected: p.protected,
            })
            .collect())
    }

    pub(super) fn v1_person_name(
        &self,
        id: PersonId,
        name: Option<String>,
    ) -> Result<(), ApiError> {
        if name
            .as_deref()
            .is_some_and(|n| n.trim().chars().count() > people::MAX_NAME_CHARS)
        {
            return Err(code("name_too_long"));
        }
        let ok =
            people::set_name(self.actions().store_mut(), id, name.as_deref()).map_err(internal)?;
        ok.then_some(()).ok_or_else(|| code("unknown_person"))
    }

    pub(super) fn v1_person_merge(
        &self,
        target: PersonId,
        absorbed: PersonId,
    ) -> Result<PersonId, ApiError> {
        match people::merge(self.actions().store_mut(), target, absorbed).map_err(internal)? {
            people::MergeOutcome::Merged(p) => Ok(p),
            people::MergeOutcome::SamePerson => Ok(target),
            people::MergeOutcome::Unknown => Err(code("unknown_person")),
        }
    }

    pub(super) fn v1_person_unmerge(&self, absorbed: PersonId) -> Result<(), ApiError> {
        let ok = people::unmerge(self.actions().store_mut(), absorbed).map_err(internal)?;
        ok.then_some(()).ok_or_else(|| code("not_merged"))
    }

    pub(super) fn v1_face_index_enabled(&self) -> bool {
        self.actions().store().face_index_enabled().unwrap_or(false)
    }

    pub(super) fn v1_face_index_set(&self, on: bool) -> Result<(), ApiError> {
        self.actions()
            .store_mut()
            .set_face_index_enabled(on)
            .map_err(internal)
    }

    /// Sletter embeddings og personer (aldri bilder). Beskyttelser av typen person blir stående som hvilende rader.
    pub(super) fn v1_face_index_wipe(&self) -> Result<u64, ApiError> {
        people::wipe(self.actions().store_mut()).map_err(internal)
    }

    pub(super) fn v1_dashboard_full(&self) -> Result<Dashboard, ApiError> {
        dashboard::read(self.actions().store()).map_err(internal)
    }

    pub(super) fn v1_dashboard(&self) -> Result<DashboardData, ApiError> {
        dashboard::data(self.actions().store()).map_err(internal)
    }

    pub(super) fn v1_settings(&self) -> Settings {
        let a = self.actions();
        Settings {
            trash_days: a.trash_days().unwrap_or(30),
            sensitivity: read_sensitivities(a.store()),
            face_index: a.store().face_index_enabled().unwrap_or(false),
        }
    }

    /// Lagrer alt eller ingenting (én feil gir uendret innstilling for resten er ikke mulig å love, så feil
    /// ignoreres og `settings()` viser det som faktisk gjelder).
    pub(super) fn v1_set_settings(&self, st: Settings) {
        let mut a = self.actions();
        let _ = a.set_trash_days(st.trash_days);
        let s = a.store_mut();
        for k in SuggestionKind::ALL {
            let _ = s.set_setting(&super::sensitivity_key(k), st.sensitivity.get(k).as_str());
        }
        let _ = s.set_setting(FACE_INDEX_KEY, if st.face_index { "1" } else { "0" });
    }

    pub(super) fn v1_thresholds(&self) -> Thresholds {
        let t = tuning_for(&read_sensitivities(self.actions().store()));
        Thresholds {
            visual_max_hamming: t.visual.max_dist,
            blur_max_score: BLUR_ABS_MAX * t.blur_scale,
            series_gap_secs: t.series.gap_secs,
            closed_eyes_min_confidence: t.series.eyes_min_confidence,
        }
    }

    /// Tall/ID bak begrunnelsesmerket for ett kort (serieposisjon, lukkede øyne, regel).
    pub(super) fn v1_card_detail(
        &self,
        store: &Store,
        kind: SuggestionKind,
        reason: ReasonCode,
        row: &CardRow,
    ) -> ReasonDetail {
        let r: Result<ReasonDetail, StoreError> =
            (|| match kind {
                SuggestionKind::Rule => {
                    Ok(rules::reason_detail(store, row.asset).unwrap_or(ReasonDetail::None))
                }
                SuggestionKind::Series => {
                    let Some(g) = row.group else {
                        return Ok(ReasonDetail::None);
                    };
                    if reason == ReasonCode::ClosedEyes {
                        let e = store.eye_results(&[row.asset])?;
                        return Ok(e
                            .get(&row.asset)
                            .map_or(ReasonDetail::None, |e| ReasonDetail::Count(e.closed as u32)));
                    }
                    let members = store.series_members(SeriesId(g.0))?;
                    let of = members.len() as u32;
                    Ok(members.iter().find(|m| m.asset == row.asset).map_or(
                        ReasonDetail::None,
                        |m| ReasonDetail::Position { index: m.rank, of },
                    ))
                }
                _ => Ok(ReasonDetail::None),
            })();
        r.unwrap_or(ReasonDetail::None)
    }

    pub(super) fn v1_series_compare(&self, series: SeriesId) -> Result<SeriesCompare, ApiError> {
        let a = self.actions();
        let store = a.store();
        let rows = store.series_members(series).map_err(internal)?;
        let Some(keeper) = rows.iter().find(|m| m.is_keeper).map(|m| m.asset) else {
            return Err(code("unknown_series"));
        };
        let ids: Vec<AssetId> = rows.iter().map(|m| m.asset).collect();
        let eyes = store.eye_results(&ids).map_err(internal)?;
        let items: Vec<SeriesItem> = store
            .index_rows_for(&ids)
            .map_err(internal)?
            .iter()
            .filter(|r| r.kind == Kind::Photo)
            .filter_map(|r| item(r, eyes.get(&r.id).copied()))
            .collect();
        let params = tuning_for(&read_sensitivities(store)).series;
        // Rangering på nytt gir begrunnelser; brukerens keeper-valg overstyrer (`with_keeper`).
        let ranked = rank_series(&items, &params)
            .filter(|s| s.members.len() == rows.len())
            .and_then(|s| s.with_keeper(keeper, &items, &params));
        let by_asset = |id: AssetId| rows.iter().find(|m| m.asset == id);
        let members = match ranked {
            Some(s) => s
                .members
                .iter()
                .enumerate()
                .filter_map(|(i, m)| {
                    let row = by_asset(m.id)?;
                    Some(SeriesMemberView {
                        asset: m.id,
                        rank: i as u32 + 1,
                        score: m.score,
                        is_keeper: m.id == keeper,
                        reason: m.reason,
                        detail: m.detail,
                        bytes: row.bytes,
                        protect_flags: row.protect,
                    })
                })
                .collect(),
            // Mangler fingeravtrykk: vis lagret rangering uten finere begrunnelse.
            None => rows
                .iter()
                .map(|m| SeriesMemberView {
                    asset: m.asset,
                    rank: m.rank,
                    score: m.score,
                    is_keeper: m.is_keeper,
                    reason: if m.is_keeper {
                        ReasonCode::KeeperBestOverall
                    } else {
                        ReasonCode::SeriesMember
                    },
                    detail: if m.is_keeper {
                        ReasonDetail::None
                    } else {
                        ReasonDetail::Position {
                            index: m.rank,
                            of: rows.len() as u32,
                        }
                    },
                    bytes: m.bytes,
                    protect_flags: m.protect,
                })
                .collect(),
        };
        Ok(SeriesCompare {
            series,
            keeper,
            members,
        })
    }
}

/// Endring av en regel (`v1_rule_change`).
#[derive(Clone, Copy)]
pub(super) enum RuleOp {
    Enabled(bool),
    Delete,
    Restore,
}
