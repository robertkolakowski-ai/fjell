//! Brukerstyrte keeper-regler på `Fjell` (se docs/mvp/REGLER.md). Del av `api` (L4).
//!
//! Grensesnitt (inherente metoder på `Fjell`, ikke på `FjellApi`):
//! - `set_keeper_rules(regler)` / `keeper_rules()`: lagrer og leser den ordnede listen (i indeksen).
//!   Lagring regner ut grupper og forslag på nytt; det påvirker bare NYE grupper og forslag.
//! - `preview_keeper_rules(regler)`: ren beregning av hvilke grupper som får ny keeper. Skriver ingenting.
//! - `apply_keeper_rules()`: «bruk regler på gjenstående grupper». Rører bare grupper uten tatte
//!   beslutninger (bare ventende forslag, ingen manuell keeper, ingen papirkurv). Reglene velger bare
//!   keeper; brukeren bekrefter forslagene som før.

use super::fjell::{internal, Fjell};
use super::ApiError;
use crate::model::{AssetId, GroupId};
use crate::pipeline::{self, RuleCtx};
use crate::rules::{decode_rules, encode_rules, KeeperRule, RuleReason};
use crate::store::{Store, StoreError, KEEPER_RULES_KEY};
use std::collections::BTreeMap;

/// Ny keeper i en gruppe. `rule`: regelen som avgjorde (`None` = automatikken K5).
#[derive(Clone, Debug, PartialEq)]
pub struct KeeperChange {
    pub group: GroupId,
    pub old_keeper: AssetId,
    pub new_keeper: AssetId,
    pub rule: Option<RuleReason>,
}

/// Lagrede regler (ugyldige linjer er alt droppet av `decode_rules`).
fn stored_rules(s: &Store) -> Result<Vec<KeeperRule>, StoreError> {
    Ok(decode_rules(
        &s.setting(KEEPER_RULES_KEY)?.unwrap_or_default(),
    ))
}

/// Ren beregning: grupper som får ny keeper med `rules`. Bare lesing.
fn changes(
    s: &Store,
    rules: &[KeeperRule],
) -> Result<Vec<(KeeperChange, crate::model::ReasonCode)>, StoreError> {
    let rows = s.index_rows()?;
    let by_id: BTreeMap<AssetId, _> = rows.iter().map(|r| (r.id, r)).collect();
    let paths = if rules.is_empty() {
        BTreeMap::new()
    } else {
        s.active_paths()?
    };
    let ctx = RuleCtx {
        rules,
        paths: &paths,
    };
    let mut out = Vec::new();
    for g in s.rule_groups()? {
        let Some(members) = g
            .members
            .iter()
            .map(|id| by_id.get(id).map(|r| pipeline::member(r)))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Some((new, reason, rule)) = pipeline::pick_keeper(&members, &ctx) else {
            continue;
        };
        // Et beskyttet bilde skal aldri få forslag, heller ikke ved bytte av keeper.
        if new == g.keeper || pipeline::is_protected(&ctx, g.keeper) {
            continue;
        }
        out.push((
            KeeperChange {
                group: g.id,
                old_keeper: g.keeper,
                new_keeper: new,
                rule,
            },
            reason,
        ));
    }
    Ok(out)
}

impl Fjell {
    /// Lagrer reglene (erstatter hele listen) og regner ut grupper og forslag på nytt.
    pub fn set_keeper_rules(&self, rules: Vec<KeeperRule>) -> Result<(), ApiError> {
        self.actions()
            .store_mut()
            .set_setting(KEEPER_RULES_KEY, &encode_rules(&rules))
            .map_err(internal)?;
        self.refresh()
    }

    pub fn keeper_rules(&self) -> Vec<KeeperRule> {
        self.with_store(|s| stored_rules(s).unwrap_or_default())
    }

    /// Hvilke grupper som får ny keeper med `rules`. Ren beregning: indeksen er uendret etterpå.
    pub fn preview_keeper_rules(
        &self,
        rules: &[KeeperRule],
    ) -> Result<Vec<KeeperChange>, ApiError> {
        let v = self.with_store(|s| changes(s, rules)).map_err(internal)?;
        Ok(v.into_iter().map(|(c, _)| c).collect())
    }

    /// Bruker de lagrede reglene på gjenstående grupper. Gir endringene som ble gjort.
    pub fn apply_keeper_rules(&self) -> Result<Vec<KeeperChange>, ApiError> {
        self.sync_groups()?;
        let mut a = self.actions();
        let rules = stored_rules(a.store()).map_err(internal)?;
        let todo = changes(a.store(), &rules).map_err(internal)?;
        let mut done = Vec::new();
        for (c, reason) in todo {
            let enc = c.rule.as_ref().map(RuleReason::encode);
            let ok = a
                .store_mut()
                .set_group_keeper_by_rule(c.group, c.new_keeper, enc.as_deref(), reason)
                .map_err(internal)?;
            if ok {
                done.push(c);
            }
        }
        Ok(done)
    }
}
