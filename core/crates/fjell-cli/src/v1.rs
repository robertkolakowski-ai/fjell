//! V1-kommandoer for `fjell-cli`: `protect`, `series`, `dashboard`, `people`, `rules`.
//!
//! Avhenger av: `flow` (`open`, `FlowError`), `fjell-core::api` (`Fjell`, `FjellApi`). Grensesnitt:
//! `run(cmd, args, out)` med `args` = `<mappe> --db <fil> [--yes] <underkommando> [argumenter]`.
//!
//! Ingen kommando sletter bilder, og kildemappen leses bare (indeksfilen ligger utenfor den):
//! - `protect list | add <asset|folder|album|person> <mål> | remove <asset|folder|album|person> <mål>`
//! - `series list | compare <serie-id>`: foreslår behold-kandidat; endrer ingenting
//! - `dashboard`: tall fra indeksen (kan ryddes, papirkurv, frigjort, antall beskyttet/serier/personer/regler)
//! - `people list | face-index [on|off|wipe]`: ansiktsindeks er av som standard. `wipe` fjerner bare avledede
//!   ansiktsdata og krever `--yes` (ellers exit 3 og ingenting skjer)
//! - `rules list | preview <vilkår...> | add <navn> <vilkår...> | toggle <id> on|off | delete <id> | restore <id>`
//!   Vilkår: `screenshot`, `blurry`, `exact-duplicate`, `older=<dager>`, `larger=<bytes>`, `smaller=<bytes>`,
//!   `album=<id>`, `folder=<prefiks>`, `mime=<type>`. Regler foreslår bare; `delete` kan angres med `restore`.
//!
//! Utskrift er `nøkkel=verdi`-linjer og koder (aldri oversatt tekst).

use crate::flow::{fail, open, w, FlowError};
use fjell_core::api::{ApiError, FjellApi, RuleDraft};
use fjell_core::model::{
    AssetId, PersonId, ProtectScope, RuleCondition, RuleId, RuleSpec, SeriesId, SuggestionKind,
};
use std::io::Write;
use std::path::PathBuf;

/// Kommandonavnene dette modulet håndterer.
pub const COMMANDS: [&str; 5] = ["protect", "series", "dashboard", "people", "rules"];

pub const USAGE: &str = "bruk: fjell-cli <protect|series|dashboard|people|rules> <mappe> --db <fil> [--yes] <underkommando> [...]\n  protect list | add|remove <asset|folder|album|person> <mål>\n  series list | compare <id>\n  dashboard\n  people list | face-index [on|off|wipe]\n  rules list | preview <vilkår...> | add <navn> <vilkår...> | toggle <id> on|off | delete <id> | restore <id>";

fn usage<T>() -> Result<T, FlowError> {
    Err(FlowError::Usage(USAGE.into()))
}

fn num(s: &str) -> Result<u64, FlowError> {
    s.parse()
        .map_err(|_| FlowError::Usage(format!("ugyldig tall: {s}")))
}

fn api(e: ApiError) -> FlowError {
    match e {
        ApiError::AckRequired(c) => FlowError::NeedsConfirm(format!("krever bekreftelse: {c:?}")),
        ApiError::Internal(c) => FlowError::Failed(c),
        other => fail(other),
    }
}

/// `asset <id>`, `folder <prefiks>`, `album <id>`, `person <id>`.
fn scope(kind: &str, target: &str) -> Result<ProtectScope, FlowError> {
    Ok(match kind {
        "asset" => ProtectScope::Asset(AssetId(num(target)?)),
        "folder" => ProtectScope::Folder(target.to_string()),
        "album" => ProtectScope::Album(num(target)?),
        "person" => ProtectScope::Person(PersonId(num(target)?)),
        _ => return usage(),
    })
}

fn scope_text(s: &ProtectScope) -> String {
    format!("{}={}", s.scope_code(), s.target())
}

/// Ett vilkår fra kommandolinjen.
pub fn condition(s: &str) -> Result<RuleCondition, FlowError> {
    let (k, v) = s.split_once('=').map_or((s, None), |(k, v)| (k, Some(v)));
    fn need<'a>(k: &str, v: Option<&'a str>) -> Result<&'a str, FlowError> {
        v.ok_or_else(|| FlowError::Usage(format!("vilkåret {k} trenger =verdi")))
    }
    Ok(match k {
        "screenshot" => RuleCondition::IsScreenshot,
        "blurry" => RuleCondition::IsBlurry,
        "exact-duplicate" => RuleCondition::IsExactDuplicate,
        "older" => RuleCondition::OlderThanDays(num(need(k, v)?)? as u32),
        "larger" => RuleCondition::LargerThanBytes(num(need(k, v)?)?),
        "smaller" => RuleCondition::SmallerThanBytes(num(need(k, v)?)?),
        "album" => RuleCondition::InAlbum(num(need(k, v)?)?),
        "folder" => RuleCondition::InFolder(need(k, v)?.to_string()),
        "mime" => RuleCondition::Mime(need(k, v)?.to_string()),
        _ => return Err(FlowError::Usage(format!("ukjent vilkår: {s}"))),
    })
}

fn spec(conds: &[String]) -> Result<RuleSpec, FlowError> {
    Ok(RuleSpec {
        all_of: conds
            .iter()
            .map(|c| condition(c))
            .collect::<Result<_, _>>()?,
    })
}

fn cond_text(c: &RuleCondition) -> String {
    match c {
        RuleCondition::IsScreenshot => "screenshot".into(),
        RuleCondition::IsBlurry => "blurry".into(),
        RuleCondition::IsExactDuplicate => "exact-duplicate".into(),
        RuleCondition::OlderThanDays(d) => format!("older={d}"),
        RuleCondition::LargerThanBytes(b) => format!("larger={b}"),
        RuleCondition::SmallerThanBytes(b) => format!("smaller={b}"),
        RuleCondition::InAlbum(a) => format!("album={a}"),
        RuleCondition::InFolder(p) => format!("folder={p}"),
        RuleCondition::Mime(m) => format!("mime={m}"),
    }
}

/// Kjører en V1-kommando. `args` er alt etter kommandonavnet.
pub fn run(cmd: &str, args: &[String], out: &mut dyn Write) -> Result<(), FlowError> {
    let (mut db, mut yes) = (None, false);
    let (mut dir, mut rest): (Option<PathBuf>, Vec<String>) = (None, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => db = it.next().map(PathBuf::from),
            "--yes" => yes = true,
            s if dir.is_none() => dir = Some(PathBuf::from(s)),
            s => rest.push(s.to_string()),
        }
    }
    let (Some(dir), Some(db)) = (dir, db) else {
        return usage();
    };
    let f = open(&dir, &db)?;
    let sub: Vec<&str> = rest.iter().map(String::as_str).collect();
    match (cmd, sub.as_slice()) {
        ("protect", ["list"]) => {
            for e in f.protections().map_err(api)? {
                w(
                    out,
                    format!(
                        "protected {} created_at={}",
                        scope_text(&e.scope),
                        e.created_at
                    ),
                )?;
            }
            Ok(())
        }
        ("protect", ["add", k, t]) => {
            f.protect_set(scope(k, t)?).map_err(api)?;
            w(out, format!("protected {k}={t}"))
        }
        ("protect", ["remove", k, t]) => {
            f.unprotect(scope(k, t)?).map_err(api)?;
            w(
                out,
                format!("unprotected {k}={t} (bildene er urørt; angre med add)"),
            )
        }
        ("series", ["list"]) => {
            let cards = f.cards(SuggestionKind::Series, 10_000, None).map_err(api)?;
            for c in &cards.cards {
                w(
                    out,
                    format!(
                        "series={} asset={} keeper={} reason={:?} detail={:?} bytes={}",
                        c.group.map_or(0, |g| g.0),
                        c.asset.0,
                        c.keeper.map_or(0, |k| k.0),
                        c.reason,
                        c.detail,
                        c.bytes
                    ),
                )?;
            }
            w(out, format!("series_cards: {}", cards.cards.len()))
        }
        ("series", ["compare", id]) => {
            let c = f.series_compare(SeriesId(num(id)?)).map_err(api)?;
            w(out, format!("series={} keeper={}", c.series.0, c.keeper.0))?;
            for m in &c.members {
                w(
                    out,
                    format!(
                        "rank={} asset={} keeper={} score={:.2} reason={:?} detail={:?} bytes={} protect={}",
                        m.rank, m.asset.0, m.is_keeper, m.score, m.reason, m.detail, m.bytes, m.protect_flags.0
                    ),
                )?;
            }
            Ok(())
        }
        ("dashboard", []) => {
            let d = f.dashboard().map_err(api)?;
            w(
                out,
                format!("active: {} ({} bytes)", d.active_count, d.active_bytes),
            )?;
            w(
                out,
                format!("trashed: {} ({} bytes)", d.trashed_count, d.trashed_bytes),
            )?;
            w(out, format!("freed_bytes: {}", d.freed_bytes))?;
            for k in &d.per_kind {
                w(
                    out,
                    format!("{}: {} ({} bytes)", k.kind.as_str(), k.count, k.bytes),
                )?;
            }
            w(
                out,
                format!(
                    "protected: {} series: {} people: {} rules_active: {}",
                    d.protected_count, d.series_count, d.people_count, d.rules_active
                ),
            )?;
            for m in &d.by_month {
                w(
                    out,
                    format!("month={} count={} bytes={}", m.year_month, m.count, m.bytes),
                )?;
            }
            for p in &d.top_folders {
                w(
                    out,
                    format!("folder={} count={} bytes={}", p.path, p.count, p.bytes),
                )?;
            }
            Ok(())
        }
        ("people", ["list"]) => {
            let list = f.people_list().map_err(api)?;
            for p in &list {
                w(
                    out,
                    format!(
                        "person={} name={} faces={} assets={} protected={}",
                        p.id.0,
                        p.name.as_deref().unwrap_or("-"),
                        p.face_count,
                        p.asset_count,
                        p.protected
                    ),
                )?;
            }
            w(out, format!("people: {}", list.len()))
        }
        ("people", ["face-index"]) => w(
            out,
            format!(
                "face_index: {}",
                if f.face_index_enabled() { "on" } else { "off" }
            ),
        ),
        ("people", ["face-index", "on"]) => {
            f.face_index_set(true).map_err(api)?;
            w(out, "face_index: on".into())
        }
        ("people", ["face-index", "off"]) => {
            f.face_index_set(false).map_err(api)?;
            w(
                out,
                "face_index: off (eksisterende ansiktsdata beholdes; bruk wipe for å fjerne dem)"
                    .into(),
            )
        }
        ("people", ["face-index", "wipe"]) => {
            if !yes {
                return Err(FlowError::NeedsConfirm(
                    "wipe fjerner alle ansiktsdata og personer (bildene beholdes) og krever --yes; ingenting slettet".into(),
                ));
            }
            let n = f.face_index_wipe().map_err(api)?;
            w(out, format!("wiped faces={n}"))
        }
        ("rules", ["list"]) => {
            for r in f.rules_list().map_err(api)? {
                let conds: Vec<String> = r.spec.all_of.iter().map(cond_text).collect();
                w(
                    out,
                    format!(
                        "rule={} name={} enabled={} hits={} conditions={}",
                        r.id.0,
                        r.name,
                        r.enabled,
                        r.hits,
                        conds.join(",")
                    ),
                )?;
            }
            Ok(())
        }
        ("rules", ["preview", conds @ ..]) => {
            let spec = spec(&conds.iter().map(|s| s.to_string()).collect::<Vec<_>>())?;
            let p = f.rule_preview(spec).map_err(api)?;
            let sample: Vec<String> = p.sample.iter().map(|a| a.0.to_string()).collect();
            w(
                out,
                format!(
                    "hits={} bytes={} sample={}",
                    p.hits,
                    p.bytes,
                    sample.join(",")
                ),
            )
        }
        ("rules", ["add", name, conds @ ..]) => {
            let spec = spec(&conds.iter().map(|s| s.to_string()).collect::<Vec<_>>())?;
            let id = f
                .rule_save(RuleDraft {
                    id: None,
                    name: name.to_string(),
                    spec,
                })
                .map_err(api)?;
            w(out, format!("rule={}", id.0))
        }
        ("rules", ["toggle", id, state @ ("on" | "off")]) => {
            f.rule_toggle(RuleId(num(id)?), *state == "on")
                .map_err(api)?;
            w(out, format!("rule={id} enabled={}", *state == "on"))
        }
        ("rules", ["delete", id]) => {
            f.rule_delete(RuleId(num(id)?)).map_err(api)?;
            w(out, format!("rule={id} deleted (angre med restore)"))
        }
        ("rules", ["restore", id]) => {
            f.rule_restore(RuleId(num(id)?)).map_err(api)?;
            w(out, format!("rule={id} restored"))
        }
        _ => usage(),
    }
}
