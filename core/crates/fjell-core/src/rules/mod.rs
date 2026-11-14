//! Modul `rules` (L2): brukerstyrte keeper-regler. Se docs/mvp/REGLER.md.
//!
//! Avhenger av: model. Rene funksjoner, ingen I/O og ingen lagring.
//! Grensesnitt:
//! - `KeeperRule`: ordnet liste av regler (`PreferFolder`, `PreferNewest`, ..., `ProtectFolder`).
//! - `RuleReason`: typet begrunnelse (enum + parametre) som UI oversetter; aldri ferdig tekst.
//! - `decide(rules, kandidater) -> Decision`: snevrer inn kandidatene regel for regel. Uavgjort
//!   overlates til K5 (`suggest::rank_group`) av kalleren (`pipeline`).
//! - `protecting_prefix(rules, sti)`: `ProtectFolder`-treff. Beskyttede bilder blir aldri foreslått
//!   flyttet; kalleren utelater dem fra forslag.
//! - `encode_rules`/`decode_rules` og `RuleReason::encode`/`decode`: stabil tekstform for lagring.
//!
//! Reglene velger bare keeper. De flytter, sletter og godtar aldri noe selv.

use crate::model::AssetId;
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

/// Samme terskel som K5 for «skarpere» (10 %).
const SHARPER_FACTOR: f64 = 1.1;

/// Én brukerregel. Rekkefølgen i listen er rangeringen (første som skiller, vinner).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeeperRule {
    /// Behold bildet fra denne mappen. Alle `PreferFolder` vurderes samlet der den første står;
    /// høyest prioritet vinner, lik prioritet gir listerekkefølge.
    PreferFolder {
        prefix: String,
        priority: i32,
    },
    PreferNewest,
    PreferOldest,
    /// Størst filstørrelse.
    PreferLargest,
    PreferHighestResolution,
    PreferSharpest,
    PreferShortestPath,
    PreferShortestName,
    /// Bilder i denne mappen kan aldri foreslås flyttet til papirkurv.
    ProtectFolder {
        prefix: String,
    },
}

/// Hvilken regel som avgjorde. UI oversetter nøkkelen (`rule.<key>`) og setter inn parameteren.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleReason {
    PreferFolder { prefix: String },
    PreferNewest,
    PreferOldest,
    PreferLargest,
    PreferHighestResolution,
    PreferSharpest,
    PreferShortestPath,
    PreferShortestName,
    ProtectedFolder { prefix: String },
}

impl RuleReason {
    /// Stabil snake_case-nøkkel.
    pub fn key(&self) -> &'static str {
        match self {
            Self::PreferFolder { .. } => "prefer_folder",
            Self::PreferNewest => "prefer_newest",
            Self::PreferOldest => "prefer_oldest",
            Self::PreferLargest => "prefer_largest",
            Self::PreferHighestResolution => "prefer_highest_resolution",
            Self::PreferSharpest => "prefer_sharpest",
            Self::PreferShortestPath => "prefer_shortest_path",
            Self::PreferShortestName => "prefer_shortest_name",
            Self::ProtectedFolder { .. } => "protected_folder",
        }
    }
    /// Mappeparameter, hvis noen.
    pub fn param(&self) -> Option<&str> {
        match self {
            Self::PreferFolder { prefix } | Self::ProtectedFolder { prefix } => Some(prefix),
            _ => None,
        }
    }
    /// Lagringsform: `nøkkel` eller `nøkkel:param` (param escapet).
    pub fn encode(&self) -> String {
        match self.param() {
            Some(p) => format!("{}:{}", self.key(), esc(p)),
            None => self.key().to_string(),
        }
    }
    pub fn decode(s: &str) -> Option<RuleReason> {
        let (key, param) = match s.split_once(':') {
            Some((k, p)) => (k, Some(unesc(p)?)),
            None => (s, None),
        };
        Some(match (key, param) {
            ("prefer_folder", Some(prefix)) => Self::PreferFolder { prefix },
            ("protected_folder", Some(prefix)) => Self::ProtectedFolder { prefix },
            ("prefer_newest", None) => Self::PreferNewest,
            ("prefer_oldest", None) => Self::PreferOldest,
            ("prefer_largest", None) => Self::PreferLargest,
            ("prefer_highest_resolution", None) => Self::PreferHighestResolution,
            ("prefer_sharpest", None) => Self::PreferSharpest,
            ("prefer_shortest_path", None) => Self::PreferShortestPath,
            ("prefer_shortest_name", None) => Self::PreferShortestName,
            _ => return None,
        })
    }
}

/// Bilde slik reglene ser det.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleCandidate {
    pub id: AssetId,
    pub path: String,
    pub created_at: i64,
    pub bytes: u64,
    pub pixels: u64,
    /// Vurderbar skarphet (`None` = ikke vurderbar).
    pub sharpness: Option<f64>,
}

/// Utfall av `decide`: gjenværende kandidater (sortert på id, aldri tom for tom input) og regelen
/// som først snevret dem inn (`None` = reglene skilte ingenting; K5 avgjør blant alle).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub narrowed: Vec<AssetId>,
    pub reason: Option<RuleReason>,
}

/// Normaliserer sti: `\` til `/`, ingen etterfølgende `/`, små bokstaver (Windows er ufølsom).
fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Ligger `path` i mappen `prefix` (hele komponenter, så `/a/bo` er ikke i `/a/b`)?
pub fn in_folder(path: &str, prefix: &str) -> bool {
    let (p, f) = (norm(path), norm(prefix));
    if f.is_empty() {
        return false;
    }
    p == f || (p.starts_with(&f) && p.as_bytes().get(f.len()) == Some(&b'/'))
}

/// Første `ProtectFolder` (i listerekkefølge) som dekker `path`; gir mappen.
pub fn protecting_prefix<'a>(rules: &'a [KeeperRule], path: &str) -> Option<&'a str> {
    rules.iter().find_map(|r| match r {
        KeeperRule::ProtectFolder { prefix } if in_folder(path, prefix) => Some(prefix.as_str()),
        _ => None,
    })
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Behold kandidatene med best nøkkel (`pick` velger beste av to). Gir `true` hvis noen falt bort.
fn keep_best<K: Ord + Copy>(
    c: &mut Vec<&RuleCandidate>,
    key: impl Fn(&RuleCandidate) -> K,
    max: bool,
) -> bool {
    let keys = c.iter().map(|m| key(m));
    let best = if max { keys.max() } else { keys.min() };
    let Some(best) = best else { return false };
    let before = c.len();
    c.retain(|m| key(m) == best);
    c.len() < before
}

/// Snevrer inn `cands` med reglene i rekkefølge. Beskyttede mapper (`ProtectFolder`) går foran alle
/// andre regler: finnes beskyttede kandidater, er bare de med. Resultatet er uavhengig av
/// rekkefølgen på `cands`.
pub fn decide(rules: &[KeeperRule], cands: &[RuleCandidate]) -> Decision {
    let mut c: Vec<&RuleCandidate> = cands.iter().collect();
    let mut reason: Option<RuleReason> = None;
    // Beskyttede mapper først.
    let prot: Vec<(&RuleCandidate, &str)> = c
        .iter()
        .filter_map(|m| protecting_prefix(rules, &m.path).map(|p| (*m, p)))
        .collect();
    if !prot.is_empty() && prot.len() < c.len() {
        let prefix = prot
            .iter()
            .min_by_key(|(m, _)| m.id)
            .map_or("", |(_, p)| *p);
        reason = Some(RuleReason::ProtectedFolder {
            prefix: prefix.to_string(),
        });
        let ids: BTreeSet<AssetId> = prot.iter().map(|(m, _)| m.id).collect();
        c.retain(|m| ids.contains(&m.id));
    }
    let mut folders_done = false;
    for rule in rules {
        let (narrowed, why) = match rule {
            KeeperRule::PreferFolder { .. } if folders_done => continue,
            KeeperRule::PreferFolder { .. } => {
                folders_done = true;
                let (n, w) = prefer_folder(rules, &mut c);
                (n, w)
            }
            KeeperRule::PreferNewest => (
                keep_best(&mut c, |m| m.created_at, true),
                RuleReason::PreferNewest,
            ),
            KeeperRule::PreferOldest => (
                keep_best(&mut c, |m| m.created_at, false),
                RuleReason::PreferOldest,
            ),
            KeeperRule::PreferLargest => (
                keep_best(&mut c, |m| m.bytes, true),
                RuleReason::PreferLargest,
            ),
            KeeperRule::PreferHighestResolution => (
                keep_best(&mut c, |m| m.pixels, true),
                RuleReason::PreferHighestResolution,
            ),
            KeeperRule::PreferSharpest => {
                let best = c
                    .iter()
                    .filter_map(|m| m.sharpness)
                    .fold(f64::MIN, f64::max);
                let before = c.len();
                c.retain(|m| m.sharpness.is_none_or(|s| s * SHARPER_FACTOR >= best));
                (c.len() < before, RuleReason::PreferSharpest)
            }
            KeeperRule::PreferShortestPath => (
                keep_best(&mut c, |m| m.path.chars().count(), false),
                RuleReason::PreferShortestPath,
            ),
            KeeperRule::PreferShortestName => (
                keep_best(&mut c, |m| file_name(&m.path).chars().count(), false),
                RuleReason::PreferShortestName,
            ),
            KeeperRule::ProtectFolder { .. } => continue,
        };
        if narrowed && reason.is_none() {
            reason = Some(why);
        }
    }
    let mut narrowed: Vec<AssetId> = c.iter().map(|m| m.id).collect();
    narrowed.sort();
    Decision { narrowed, reason }
}

/// Samlet vurdering av alle `PreferFolder`: høyest prioritet blant treff vinner.
fn prefer_folder(rules: &[KeeperRule], c: &mut Vec<&RuleCandidate>) -> (bool, RuleReason) {
    let score = |m: &RuleCandidate| -> Option<(i32, usize)> {
        // (prioritet, -listeindeks) via max: lavest indeks vinner ved lik prioritet.
        rules
            .iter()
            .enumerate()
            .filter_map(|(i, r)| match r {
                KeeperRule::PreferFolder { prefix, priority } if in_folder(&m.path, prefix) => {
                    Some((*priority, usize::MAX - i))
                }
                _ => None,
            })
            .max()
    };
    let Some(best) = c.iter().filter_map(|m| score(m)).max() else {
        return (
            false,
            RuleReason::PreferFolder {
                prefix: String::new(),
            },
        );
    };
    let prefix = match &rules[usize::MAX - best.1] {
        KeeperRule::PreferFolder { prefix, .. } => prefix.clone(),
        _ => String::new(),
    };
    let before = c.len();
    c.retain(|m| score(m).is_some_and(|s| s.0 == best.0));
    (c.len() < before, RuleReason::PreferFolder { prefix })
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '%' => o.push_str("%25"),
            '\t' => o.push_str("%09"),
            '\n' => o.push_str("%0A"),
            '\r' => o.push_str("%0D"),
            ':' => o.push_str("%3A"),
            _ => o.push(ch),
        }
    }
    o
}

fn unesc(s: &str) -> Option<String> {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(ch) = it.next() {
        if ch != '%' {
            o.push(ch);
            continue;
        }
        let hex: String = it.by_ref().take(2).collect();
        o.push(u8::from_str_radix(&hex, 16).ok()? as char);
    }
    Some(o)
}

/// Lagringsform: én regel per linje, felt skilt med tab: `type[\tparam[\tprioritet]]`.
pub fn encode_rules(rules: &[KeeperRule]) -> String {
    let mut out = String::new();
    for r in rules {
        let line = match r {
            KeeperRule::PreferFolder { prefix, priority } => {
                format!("prefer_folder\t{}\t{priority}", esc(prefix))
            }
            KeeperRule::PreferNewest => "prefer_newest".into(),
            KeeperRule::PreferOldest => "prefer_oldest".into(),
            KeeperRule::PreferLargest => "prefer_largest".into(),
            KeeperRule::PreferHighestResolution => "prefer_highest_resolution".into(),
            KeeperRule::PreferSharpest => "prefer_sharpest".into(),
            KeeperRule::PreferShortestPath => "prefer_shortest_path".into(),
            KeeperRule::PreferShortestName => "prefer_shortest_name".into(),
            KeeperRule::ProtectFolder { prefix } => format!("protect_folder\t{}", esc(prefix)),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Motsatt av `encode_rules`. Ukjente eller ødelagte linjer hoppes over (aldri panikk, aldri gjetting).
pub fn decode_rules(s: &str) -> Vec<KeeperRule> {
    s.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            Some(match f.as_slice() {
                ["prefer_folder", p, n] => KeeperRule::PreferFolder {
                    prefix: unesc(p)?,
                    priority: n.parse().ok()?,
                },
                ["protect_folder", p] => KeeperRule::ProtectFolder { prefix: unesc(p)? },
                ["prefer_newest"] => KeeperRule::PreferNewest,
                ["prefer_oldest"] => KeeperRule::PreferOldest,
                ["prefer_largest"] => KeeperRule::PreferLargest,
                ["prefer_highest_resolution"] => KeeperRule::PreferHighestResolution,
                ["prefer_sharpest"] => KeeperRule::PreferSharpest,
                ["prefer_shortest_path"] => KeeperRule::PreferShortestPath,
                ["prefer_shortest_name"] => KeeperRule::PreferShortestName,
                _ => return None,
            })
        })
        .collect()
}
