//! Felles testoppsett for `actions`: fixture, fake plattform og sjekksummer.
//!
//! Grensesnitt: `Fx` (Actions + FakeClock), `Fx::asset/group/suggest/album`, `FakePlatform`
//! (logger `delete`-kall, I1), `dump_sum` (sjekksum av alle tabeller, brukes for I4 og I6).
//! Undermoduler: `scenarios` (T1-T10), `scenarios2` (T11-T20), `props` (I4-I16), `sim` (modelltest
//! med I1-I3, I5, I9-I11, I14-I15), `perf` (50 000 bilder).

mod perf;
mod props;
mod scenarios;
mod scenarios2;
mod sim;

use super::*;
use crate::model::{
    Acks, AssetId, ConsequenceCode, DeleteOutcome, FakeClock, PlatformDelete, PurgeTicket,
    SourceId, State,
};
use std::sync::{Arc, Mutex};

pub const DAY: i64 = 86_400;

pub struct Fx {
    pub a: Actions,
    pub clock: Arc<FakeClock>,
}

impl Fx {
    pub fn new() -> Fx {
        Fx::with_store(Store::open_in_memory().unwrap())
    }

    pub fn with_store(store: Store) -> Fx {
        let clock = Arc::new(FakeClock::new(1_000_000));
        Fx {
            a: Actions::new(store, clock.clone()),
            clock,
        }
    }

    /// Setter inn en aktiv asset. `protect` er bitmaske, `backup` 0/1/None.
    pub fn asset(&self, id: u64, bytes: u64, protect: u8, backup: Option<i64>) -> AssetId {
        self.a
            .store()
            .conn()
            .execute(
                "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime, \
                 protect_flags, backup) VALUES (?1, ?2, 'photo', 1, 'm', 4, 4, ?3, 'image/jpeg', ?4, ?5)",
                rusqlite::params![id as i64, format!("s{id}"), bytes as i64, protect, backup],
            )
            .unwrap();
        AssetId(id)
    }

    /// `n` enkle aktive bilder med id 1..=n (3 byte hver).
    pub fn assets(&self, n: u64) -> Vec<AssetId> {
        (1..=n).map(|i| self.asset(i, 3, 0, None)).collect()
    }

    /// Gruppe med medlemmer i rangordning (første = keeper).
    pub fn group(&self, gid: i64, members: &[AssetId]) {
        let c = self.a.store().conn();
        c.execute(
            "INSERT INTO grp(id, kind, keeper_id, created_at) VALUES (?1, 'visual', ?2, 1)",
            rusqlite::params![gid, members[0].0 as i64],
        )
        .unwrap();
        for (rank, m) in members.iter().enumerate() {
            c.execute(
                "INSERT INTO grp_member(grp_id, asset_id, rank) VALUES (?1, ?2, ?3)",
                rusqlite::params![gid, m.0 as i64, rank as i64],
            )
            .unwrap();
        }
    }

    pub fn suggest(&self, id: AssetId, kind: &str, grp: Option<i64>) {
        self.a
            .store()
            .conn()
            .execute(
                "INSERT INTO suggestion(asset_id, kind, grp_id, reason_code, confidence) VALUES (?1, ?2, ?3, 'visual_duplicate', 0.9)",
                rusqlite::params![id.0 as i64, kind, grp],
            )
            .unwrap();
    }

    pub fn album(&self, album: i64, name: &str, shared: bool, members: &[AssetId]) {
        let c = self.a.store().conn();
        c.execute(
            "INSERT INTO album(id, name, shared) VALUES (?1, ?2, ?3)",
            rusqlite::params![album, name, shared],
        )
        .unwrap();
        for m in members {
            c.execute(
                "INSERT INTO asset_album VALUES (?1, ?2)",
                rusqlite::params![m.0 as i64, album],
            )
            .unwrap();
        }
    }

    pub fn state(&self, id: AssetId) -> State {
        self.a.store().asset_row(id).unwrap().unwrap().state
    }

    pub fn states(&self, ids: &[AssetId]) -> Vec<State> {
        ids.iter().map(|&i| self.state(i)).collect()
    }

    pub fn log_count(&self) -> i64 {
        self.sql_i64("SELECT COUNT(*) FROM action_log")
    }

    pub fn sql_i64(&self, sql: &str) -> i64 {
        self.a
            .store()
            .conn()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }

    pub fn total_changes(&self) -> u64 {
        self.a.store().conn().total_changes()
    }

    /// Trasher mengden med alle nødvendige acks.
    pub fn trash(&mut self, ids: &[AssetId]) -> Option<crate::model::BatchId> {
        let plan = self.a.plan(&PlanRequest::manual(ids)).unwrap();
        self.a.commit(&plan, &acks_covering(&plan)).unwrap()
    }

    /// Trash -> begin -> confirm -> platformsletting -> finish (som UI), alt `Deleted`.
    pub fn purge_all(&mut self, ids: &[AssetId], plat: &FakePlatform) -> PurgeOutcome {
        let offer = self.a.begin_purge(ids).unwrap();
        let ticket = self
            .a
            .confirm_purge(&offer, Some(&crate::model::MassAck::new(offer.digest)))
            .unwrap();
        let res = plat
            .delete_checked(&self.a, &ticket, DeleteOutcome::Deleted)
            .unwrap();
        self.a.finish_purge(ticket, &res).unwrap()
    }
}

/// Fake plattformsadapter: logger `delete`-kall og husker «slettede» kilder (I1, T9-T11).
#[derive(Default)]
pub struct FakePlatform {
    pub calls: Mutex<Vec<(u64, usize)>>,
    pub gone: Mutex<Vec<SourceId>>,
}

impl PlatformDelete for FakePlatform {
    fn delete(
        &self,
        ticket: &PurgeTicket,
        ids: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, DeleteOutcome)> {
        self.calls
            .lock()
            .unwrap()
            .push((ticket.confirmed().digest(), ids.len()));
        self.gone
            .lock()
            .unwrap()
            .extend(ids.iter().map(|(_, s)| s.clone()));
        ids.iter()
            .map(|(i, _)| (*i, DeleteOutcome::Deleted))
            .collect()
    }
}

impl FakePlatform {
    /// UI-flyt: sjekk billetten først (T16), så `delete`, med gitt utfall for alle.
    pub fn delete_checked(
        &self,
        a: &Actions,
        ticket: &PurgeTicket,
        outcome: DeleteOutcome,
    ) -> Result<Vec<(AssetId, DeleteOutcome)>> {
        a.check_ticket(ticket)?;
        let ids: Vec<_> = ticket
            .ids()
            .iter()
            .map(|&i| (i, SourceId(format!("s{}", i.0))))
            .collect();
        let res = self.delete(ticket, &ids);
        Ok(res.into_iter().map(|(i, _)| (i, outcome)).collect())
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

/// Sjekksum av alle rader i alle tabeller (for «ingenting er skrevet»).
pub fn dump_sum(a: &Actions) -> u64 {
    let c = a.store().conn();
    let mut h: u64 = 1469598103934665603;
    for t in [
        "asset",
        "fingerprint",
        "grp",
        "grp_member",
        "suggestion",
        "action_log",
        "setting",
        "album",
        "asset_album",
    ] {
        let mut st = c
            .prepare(&format!("SELECT * FROM {t} ORDER BY 1, 2"))
            .unwrap();
        let n = st.column_count();
        let mut rows = st.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            for i in 0..n {
                let v: rusqlite::types::Value = r.get(i).unwrap();
                for b in format!("{v:?}").bytes() {
                    h = (h ^ b as u64).wrapping_mul(1099511628211);
                }
            }
        }
    }
    h
}

pub fn assert_ack_required(r: Result<MaybeBatch>, want: &[ConsequenceCode]) {
    match r {
        Err(ActionError::AckRequired(c)) => assert_eq!(c, want),
        other => panic!("forventet AckRequired({want:?}), fikk {other:?}"),
    }
}
