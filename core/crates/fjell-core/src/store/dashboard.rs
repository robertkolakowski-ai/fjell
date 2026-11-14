//! Lagring for V1-grenen `dashboard` (store, L1). Eier aggregeringsspørringene for `DashboardData`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `dashboard` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Bare lesing; ingen skriving til indeksen.
//!
//! Grensesnitt (alle `&self`, alle bare `SELECT`):
//! - `dash_protected_ids`: ID-er beskyttet via `protection` (enkeltbilde, album, person, mappe); bygges én gang
//!   og gis til `dash_totals` og `dash_reclaimable`.
//! - `active_asset_count`, `dash_totals`: brukt plass, papirkurv og antall beskyttede.
//! - `dash_reclaimable`: ventende, synlige forslag per type, **uten** beskyttede bilder; hvert bilde teller
//!   under nøyaktig én type (total rekkefølge), så summen over typer er lik summen over bilder.
//! - `dash_distribution`: aktive bilder per måned og de største mappene (ett pass, uten SQL-strengfunksjoner).
//! - `dash_counts`: serier, personer, aktive regler.
//! - `dash_freed_total`, `dash_freed_by_month`, `dash_freed_since`: permanent frigjort, fra `action_log`,
//!   aldri `cause = 'external'`.
//! - `dash_progress`: avgjorte og totale forslag.
//!
//! Beskyttelse: et bilde regnes som beskyttet hvis `protect_flags <> 0`, eller det finnes en `protection`-rad
//! for bildet, for et mappeprefiks av `source_id`, for et album bildet er med i, eller for en person (inkl.
//! sammenslåtte personer) med ansikt i bildet. Dette er bevisst konservativt: heller for mye enn for lite
//! beskyttet. Ansiktsbeskyttelse som henger etter «Slett ansiktsindeks» har ingen embeddings å treffe.
//!
//! Ytelse (budsjett i `dashboard/tests.rs`): de beskyttede bilde-ID-ene bygges fra indeksene i `protection`,
//! `asset_album`, `face_embedding` og `asset.source_id` (arbeid proporsjonalt med det som er beskyttet, ikke med
//! biblioteket). Resten av biblioteket sjekkes med ett `IN`-oppslag per rad.

use super::{Store, StoreError};
use crate::model::SuggestionKind;
use std::collections::{HashMap, HashSet};

/// Beskyttede bilde-ID-er (`prot_ids`), bygget fra indeksene. `pp` er beskyttede personer, inkludert alle som er
/// slått sammen inn i en beskyttet person (rekursivt). Mappeprefiks bruker områdesøk på `source_id`-indeksen
/// (alle strenger med prefikset er mindre enn prefikset + U+10FFFF); `substr`-sjekken gjør treffet nøyaktig.
const PROT_CTE: &str = "WITH RECURSIVE \
     pp(id) AS MATERIALIZED ( \
       SELECT CAST(target AS INTEGER) FROM protection WHERE scope = 'person' \
       UNION SELECT p.id FROM person p JOIN pp ON p.merged_into = pp.id), \
     prot_ids(id) AS MATERIALIZED ( \
       SELECT CAST(target AS INTEGER) FROM protection WHERE scope = 'asset' \
       UNION ALL SELECT aa.asset_id FROM protection p JOIN asset_album aa ON aa.album_id = CAST(p.target AS INTEGER) \
             WHERE p.scope = 'album' \
       UNION ALL SELECT f.asset_id FROM face_embedding f WHERE f.person_id IN (SELECT id FROM pp) \
       UNION ALL SELECT a2.id FROM protection p JOIN asset a2 \
             ON a2.source_id >= p.target AND a2.source_id < p.target || char(1114111) \
            AND substr(a2.source_id, 1, length(p.target)) = p.target \
             WHERE p.scope = 'folder') ";

/// Total rekkefølge på forslagstyper (lavest vinner), slik at hvert bilde havner under én type.
const RANKED: [SuggestionKind; 6] = [
    SuggestionKind::ExactDup,
    SuggestionKind::VisualDup,
    SuggestionKind::Screenshot,
    SuggestionKind::Blurry,
    SuggestionKind::Series,
    SuggestionKind::Rule,
];

/// Rang for `suggestion.kind`; ukjent tekst får lavest prioritet (siste rang).
fn rank_of(kind: &str) -> u8 {
    RANKED
        .iter()
        .position(|k| k.as_str() == kind)
        .unwrap_or(RANKED.len() - 1) as u8
}

/// Feil ved lesing av en kolonne med uventet type (aldri forventet; skjemaet har NOT NULL og typer).
fn conv(e: rusqlite::types::FromSqlError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Null, Box::new(e))
}

fn u(v: i64) -> u64 {
    v.max(0) as u64
}

/// Brukt plass, papirkurv og beskyttede, fra ett pass over `asset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Totals {
    pub used_count: u64,
    pub used_bytes: u64,
    pub trashed_count: u64,
    pub trashed_bytes: u64,
    /// Aktive, beskyttede bilder.
    pub protected: u64,
}

/// Aktive bilder per måned og de største mappene.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Distribution {
    /// (`ÅÅÅÅMM`, antall, byte), eldst først.
    pub by_month: Vec<(u32, u64, u64)>,
    /// (mappesti, antall, byte), størst først.
    pub top_folders: Vec<(String, u64, u64)>,
}

/// `ÅÅÅÅMM` (UTC) for Unix-sekunder. Samme resultat som SQLite `strftime('%Y%m', ts, 'unixepoch')` for år 0001-9999
/// (sivil dato fra dagtall, Howard Hinnant). Tidspunkt utenfor området gir 0.
pub(crate) fn year_month(ts: i64) -> u32 {
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    if (1..=9999).contains(&y) {
        (y * 100 + m) as u32
    } else {
        0
    }
}

impl Store {
    /// Endringsstempel for indeksen: (endringer gjort på denne tilkoblingen, `PRAGMA data_version`). Stempelet
    /// endres ved hver skriving, også fra andre tilkoblinger, og brukes til å gjenbruke et dashboard-svar.
    pub fn dash_stamp(&self) -> Result<(u64, i64), StoreError> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA data_version", [], |r| r.get(0))?;
        Ok((self.conn.total_changes(), version))
    }

    /// Antall aktive bilder (`state = 0`).
    pub fn active_asset_count(&self) -> Result<u64, StoreError> {
        Ok(self.dash_totals(&HashSet::new())?.used_count)
    }

    /// ID-ene til alle beskyttede bilder (alle tilstander) via `protection`: enkeltbilde, album, person (inkl.
    /// sammenslåtte) og mappeprefiks. Bilder med `protect_flags <> 0` er ikke med; de leses fra `asset`.
    pub fn dash_protected_ids(&self) -> Result<HashSet<i64>, StoreError> {
        let sql = format!("{PROT_CTE}SELECT id FROM prot_ids");
        let mut st = self.conn.prepare_cached(&sql)?;
        let mut rows = st.query([])?;
        let mut out = HashSet::new();
        while let Some(r) = rows.next()? {
            out.insert(r.get(0)?);
        }
        Ok(out)
    }

    /// Brukt plass (aktive), papirkurv («Nylig slettet»; plassen er ikke frigjort før «Tøm») og antall
    /// beskyttede aktive bilder.
    pub fn dash_totals(&self, prot: &HashSet<i64>) -> Result<Totals, StoreError> {
        // Ett aggregatpass over asset; flaggbeskyttede telles i samme pass.
        let (uc, ub, tc, tb, flagged): (i64, i64, i64, i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(state = 0), 0), COALESCE(SUM(CASE WHEN state = 0 THEN bytes END), 0), \
                    COALESCE(SUM(state = 1), 0), COALESCE(SUM(CASE WHEN state = 1 THEN bytes END), 0), \
                    COALESCE(SUM(state = 0 AND protect_flags <> 0), 0) \
             FROM asset WHERE state < 2",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                ))
            },
        )?;
        // Beskyttet via ID (enkeltbilde, album, person, mappe) uten flagg. Oppslag per beskyttet ID i SQLite
        // (`json_each` driver; PK-oppslag på asset), proporsjonalt med antall beskyttede, ikke med biblioteket.
        let mut by_id = 0u64;
        if !prot.is_empty() {
            let mut json = String::with_capacity(prot.len() * 8 + 2);
            json.push('[');
            for (i, id) in prot.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&id.to_string());
            }
            json.push(']');
            let n: i64 = self.conn.query_row(
                // `CROSS JOIN` låser rekkefølgen (json_each ytterst); uten den skanner planleggeren asset.
                "SELECT COUNT(*) FROM json_each(?1) j CROSS JOIN asset a ON a.id = j.value \
                 WHERE a.state = 0 AND a.protect_flags = 0",
                [json],
                |r| r.get(0),
            )?;
            by_id = u(n);
        }
        Ok(Totals {
            used_count: u(uc),
            used_bytes: u(ub),
            trashed_count: u(tc),
            trashed_bytes: u(tb),
            protected: u(flagged) + by_id,
        })
    }

    /// (type, antall, byte) for ventende forslag på aktive, **ubeskyttede** bilder. Hvert bilde teller
    /// under nøyaktig én type (den med lavest rang blant bildets ventende forslag). Bare typer med treff.
    ///
    /// Gjøres i Rust over to enkle spørringer (beskyttede ID-er og ventende forslag på ikke-flaggede bilder):
    /// korrelerte `NOT EXISTS` over `suggestion` ble valgt av SQLite med kvadratisk skanning.
    pub fn dash_reclaimable(
        &self,
        prot: &HashSet<i64>,
    ) -> Result<Vec<(SuggestionKind, u64, u64)>, StoreError> {
        // Per bilde: (laveste rang, byte).
        let mut best: HashMap<i64, (u8, u64)> = HashMap::new();
        let mut st = self.conn.prepare_cached(
            "SELECT s.asset_id, s.kind, a.bytes FROM suggestion s JOIN asset a ON a.id = s.asset_id \
             WHERE s.status = 'pending' AND a.state = 0 AND a.protect_flags = 0",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            if prot.contains(&id) {
                continue;
            }
            let rk = rank_of(r.get_ref(1)?.as_str().map_err(conv)?);
            let bytes = u(r.get(2)?);
            let e = best.entry(id).or_insert((rk, bytes));
            e.0 = e.0.min(rk);
        }
        let mut per = [(0u64, 0u64); RANKED.len()];
        for (rk, bytes) in best.into_values() {
            per[rk as usize].0 += 1;
            per[rk as usize].1 += bytes;
        }
        Ok(RANKED
            .iter()
            .zip(per)
            .filter(|(_, p)| p.0 > 0)
            .map(|(k, p)| (*k, p.0, p.1))
            .collect())
    }

    /// Aktive bilder per opprettelsesmåned (UTC, eldst først) og de `limit` største mappene (størst først,
    /// så sti). Mappe = `source_id` foran siste `/` eller `\` (stien vises som lagret); bilder uten mappe
    /// utelates. Ett pass i Rust: SQL-strengfunksjoner per rad er mange ganger tregere.
    pub fn dash_distribution(&self, limit: usize) -> Result<Distribution, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT source_id, created_at, bytes FROM asset WHERE state = 0")?;
        let mut rows = st.query([])?;
        let mut months: HashMap<u32, (u64, u64)> = HashMap::new();
        let mut folders: HashMap<String, (u64, u64)> = HashMap::new();
        while let Some(r) = rows.next()? {
            let src = r.get_ref(0)?.as_str().map_err(conv)?;
            let bytes = u(r.get_ref(2)?.as_i64().map_err(conv)?);
            let m = months
                .entry(year_month(r.get_ref(1)?.as_i64().map_err(conv)?))
                .or_default();
            m.0 += 1;
            m.1 += bytes;
            if let Some(i) = src.rfind(['/', '\\']) {
                if i > 0 {
                    let dir = &src[..i];
                    // Slå opp uten å allokere; bare første bilde i en mappe lager en nøkkel.
                    let f = match folders.get_mut(dir) {
                        Some(f) => f,
                        None => folders.entry(dir.to_string()).or_default(),
                    };
                    f.0 += 1;
                    f.1 += bytes;
                }
            }
        }
        let mut by_month: Vec<_> = months.into_iter().map(|(k, v)| (k, v.0, v.1)).collect();
        by_month.sort_unstable();
        let mut top_folders: Vec<_> = folders.into_iter().map(|(k, v)| (k, v.0, v.1)).collect();
        top_folders.sort_unstable_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        top_folders.truncate(limit);
        Ok(Distribution {
            by_month,
            top_folders,
        })
    }

    /// (serier, selvstendige personer, aktive regler). Aktiv regel = på og ikke (mykt) slettet.
    pub fn dash_counts(&self) -> Result<(u64, u64, u64), StoreError> {
        let one = |sql: &str| -> Result<u64, StoreError> {
            Ok(u(self.conn.query_row(sql, [], |r| r.get::<_, i64>(0))?))
        };
        Ok((
            one("SELECT COUNT(*) FROM grp WHERE kind = 'series'")?,
            one("SELECT COUNT(*) FROM person WHERE merged_into IS NULL")?,
            one("SELECT COUNT(*) FROM rule WHERE enabled = 1 AND deleted_at IS NULL")?,
        ))
    }

    /// (antall, byte) permanent frigjort: `purge`-rader i `action_log`, aldri `cause = 'external'`.
    pub fn dash_freed_total(&self) -> Result<(u64, u64), StoreError> {
        self.dash_freed_since(0)
    }

    /// Som `dash_freed_total`, men bare rader med `ts >= since`.
    pub fn dash_freed_since(&self, since: i64) -> Result<(u64, u64), StoreError> {
        let (n, b): (i64, i64) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(a.bytes), 0) FROM action_log l JOIN asset a ON a.id = l.asset_id \
             WHERE l.op = 'purge' AND l.cause <> 'external' AND l.ts >= ?1",
            [since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((u(n), u(b)))
    }

    /// (`ÅÅÅÅMM`, antall, byte) frigjort per måned (UTC), eldst først. Ikke `cause = 'external'`.
    pub fn dash_freed_by_month(&self) -> Result<Vec<(u32, u64, u64)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT l.ts, a.bytes FROM action_log l JOIN asset a ON a.id = l.asset_id \
             WHERE l.op = 'purge' AND l.cause <> 'external'",
        )?;
        let mut months: HashMap<u32, (u64, u64)> = HashMap::new();
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let m = months.entry(year_month(r.get(0)?)).or_default();
            m.0 += 1;
            m.1 += u(r.get(1)?);
        }
        let mut out: Vec<_> = months.into_iter().map(|(k, v)| (k, v.0, v.1)).collect();
        out.sort_unstable();
        Ok(out)
    }

    /// (avgjorte, totale) forslag: avgjort = `accepted` eller `rejected`.
    pub fn dash_progress(&self) -> Result<(u64, u64), StoreError> {
        let (d, t): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(status <> 'pending'), 0), COUNT(*) FROM suggestion",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((u(d), u(t)))
    }
}

#[cfg(test)]
mod tests {
    use super::year_month;

    #[test]
    fn year_month_matches_known_dates_and_edges() {
        assert_eq!(year_month(0), 197_001);
        assert_eq!(year_month(1_700_000_000), 202_311);
        assert_eq!(year_month(1_703_000_000), 202_312);
        assert_eq!(year_month(951_782_400), 200_002); // 2000-02-29
        assert_eq!(year_month(-1), 196_912);
        assert_eq!(year_month(i64::MAX), 0);
    }

    /// Mot SQLite for et utvalg tidspunkt (fanger skuddår og månedsskifter).
    #[test]
    fn year_month_agrees_with_sqlite() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        let mut ts = -2_000_000_000i64;
        while ts < 4_000_000_000 {
            let want: i64 = c
                .query_row(
                    "SELECT CAST(strftime('%Y%m', ?1, 'unixepoch') AS INTEGER)",
                    [ts],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(i64::from(year_month(ts)), want, "ts {ts}");
            ts += 86_400 * 13 + 3_601;
        }
    }
}
