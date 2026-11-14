//! Keeper-valg med brukerregler (se docs/mvp/REGLER.md). Del av `pipeline` (L3).
//!
//! Avhenger av: rules, suggest, quality, model. Grensesnitt:
//! - `RuleCtx { rules, paths }`: regelsettet og stiene (`asset.source_id`) reglene trenger.
//! - `pick_group(kind, members, ctx) -> Option<Pick>`: uten regler nøyaktig `suggest::group_suggestions`
//!   (K5 uendret). Med regler: beskyttede mapper først, så reglene i rekkefølge, så K5 blant de som
//!   står igjen. Beskyttede bilder som ikke blir keeper utelates (`Pick::omitted`): de får aldri forslag.
//! - `is_protected(ctx, id)`: for forslag uten gruppe (uskarp, skjermbilde).
//!
//! Reglene velger bare keeper og utelater; ingenting flyttes eller godtas her.

use crate::model::AssetId;
use crate::quality::LOW_CONTRAST;
use crate::rules::{decide, protecting_prefix, KeeperRule, RuleCandidate, RuleReason};
use crate::suggest::{
    group_suggestions, group_suggestions_with, rank_group, GroupKind, GroupResult, Member,
};
use std::collections::{BTreeMap, BTreeSet};

/// Regler og stier. Tom regelliste gir K5 uendret.
#[derive(Clone, Copy, Debug)]
pub struct RuleCtx<'a> {
    pub rules: &'a [KeeperRule],
    pub paths: &'a BTreeMap<AssetId, String>,
}

static NO_PATHS: BTreeMap<AssetId, String> = BTreeMap::new();

impl RuleCtx<'static> {
    /// Ingen regler: K5 alene.
    pub const NONE: RuleCtx<'static> = RuleCtx {
        rules: &[],
        paths: &NO_PATHS,
    };
}

/// Rangert gruppe med eventuell avgjørende regel.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub result: GroupResult,
    /// Regelen som først skilte kandidatene (`None` = K5 alene).
    pub rule: Option<RuleReason>,
    /// Beskyttede medlemmer som ikke er keeper: utelatt fra gruppen og fra forslag.
    pub omitted: Vec<AssetId>,
}

fn path<'a>(ctx: &RuleCtx<'a>, id: AssetId) -> &'a str {
    ctx.paths.get(&id).map_or("", String::as_str)
}

/// Ligger bildet i en beskyttet mappe?
pub fn is_protected(ctx: &RuleCtx, id: AssetId) -> bool {
    protecting_prefix(ctx.rules, path(ctx, id)).is_some()
}

fn candidate(ctx: &RuleCtx, m: &Member) -> RuleCandidate {
    RuleCandidate {
        id: m.id,
        path: path(ctx, m.id).to_string(),
        created_at: m.created_at,
        bytes: m.bytes,
        pixels: m.width as u64 * m.height as u64,
        sharpness: (m.blur_score < LOW_CONTRAST).then_some(m.blur_score as f64),
    }
}

/// Keeper og begrunnelse uten å bygge forslag (brukes av forhåndsvisning og «bruk regler»).
/// `None` for tom gruppe eller når alle medlemmer er beskyttet.
pub fn pick_keeper(
    members: &[Member],
    ctx: &RuleCtx,
) -> Option<(AssetId, crate::model::ReasonCode, Option<RuleReason>)> {
    if members.is_empty() {
        return None;
    }
    if ctx.rules.is_empty() {
        let r = rank_group(members)?;
        return Some((r.keeper, r.reason, None));
    }
    let prot = members.iter().filter(|m| is_protected(ctx, m.id)).count();
    if prot == members.len() {
        return None;
    }
    let cands: Vec<RuleCandidate> = members.iter().map(|m| candidate(ctx, m)).collect();
    let d = decide(ctx.rules, &cands);
    let keep: BTreeSet<AssetId> = d.narrowed.iter().copied().collect();
    let narrowed: Vec<Member> = members
        .iter()
        .filter(|m| keep.contains(&m.id))
        .copied()
        .collect();
    let r = rank_group(&narrowed)?;
    Some((r.keeper, r.reason, d.reason))
}

pub fn pick_group(kind: GroupKind, members: &[Member], ctx: &RuleCtx) -> Option<Pick> {
    if ctx.rules.is_empty() {
        let result = group_suggestions(kind, members)?;
        return Some(Pick {
            result,
            rule: None,
            omitted: Vec::new(),
        });
    }
    let (keeper, reason, rule) = pick_keeper(members, ctx)?;
    let omitted: Vec<AssetId> = members
        .iter()
        .filter(|m| m.id != keeper && is_protected(ctx, m.id))
        .map(|m| m.id)
        .collect();
    let kept: Vec<Member> = members
        .iter()
        .filter(|m| !omitted.contains(&m.id))
        .copied()
        .collect();
    if kept.len() < 2 {
        return None;
    }
    let ranked = crate::suggest::Ranked { keeper, reason };
    let result = group_suggestions_with(kind, &kept, ranked)?;
    Some(Pick {
        result,
        rule,
        omitted,
    })
}
