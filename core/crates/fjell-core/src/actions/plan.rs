//! `plan` og `plan_pending`: ren forhåndsvisning (PLAN-TRYGGHET 4.1 og 4.4). Del av `actions`.
//!
//! Grensesnitt: `Actions::plan(&PlanRequest) -> Plan`, `Actions::plan_pending() -> Plan`,
//! `acks_covering(&Plan) -> Acks` (crate-internt: det `api` lager etter brukerens bekreftelse).
//! `plan` tar `&self` og skriver aldri (I6). Vakt: ingen gruppe mister alle aktive medlemmer (I8).

use super::digest::{digest_rows, Snapshot};
use super::{ActionError, Actions, PlanRequest, Result, Selection};
use crate::model::plan::ACK_CATEGORIES;
use crate::model::{
    Acks, AlbumCount, AssetId, BackupSummary, ConsequenceCode, ConsequenceSummary, KindCounts,
    MassAck, Plan, PlannedItem, ProtectCounts, ProtectFlags, ProtectedAck, ReasonCode, State,
    SuggestionKind,
};
use std::collections::{BTreeMap, HashMap, HashSet};

fn parse_reason(code: &str) -> Option<ReasonCode> {
    ReasonCode::ALL.into_iter().find(|r| r.key() == code)
}

impl Actions {
    /// Ren funksjon på indeksdata: gir nøyaktig det `commit` vil endre.
    pub fn plan(&self, req: &PlanRequest) -> Result<Plan> {
        // Første valg per id vinner; allerede trashed hoppes over (idempotens, I11).
        let mut seen = HashSet::new();
        let mut picks: Vec<Selection> = Vec::new();
        for s in &req.items {
            if seen.insert(s.id) {
                picks.push(*s);
            }
        }
        picks.sort_by_key(|s| s.id);
        let ids: Vec<AssetId> = picks.iter().map(|s| s.id).collect();
        let found = self.store.asset_rows(&ids)?;
        let mut reasons: HashMap<(AssetId, SuggestionKind), String> = HashMap::new();
        if picks.iter().any(|s| s.kind.is_some() && s.reason.is_none()) {
            for (id, kind, code) in self.store.reasons_for(&ids)? {
                reasons.entry((id, kind)).or_insert(code);
            }
        }
        let mut items = Vec::new();
        let mut rows = Vec::new();
        for (s, row) in picks.iter().zip(found) {
            let row = row.ok_or(ActionError::UnknownAsset(s.id))?;
            match row.state {
                State::Purged => return Err(ActionError::AlreadyPurged(s.id)),
                State::Trashed => continue,
                State::Active => {}
            }
            let reason = s.reason.or_else(|| {
                s.kind
                    .and_then(|k| reasons.get(&(s.id, k)))
                    .and_then(|c| parse_reason(c))
            });
            items.push(PlannedItem {
                id: s.id,
                kind: s.kind,
                reason,
                protect: row.protect,
            });
            rows.push(row);
        }
        self.guard_groups(&items)?;
        let snap = Snapshot {
            digest: digest_rows(&rows),
            bytes: rows.iter().map(|r| r.bytes).sum(),
            rows,
        };
        let summary = self.summarize(&items, &snap)?;
        Ok(Plan {
            items,
            summary,
            digest: snap.digest,
            created_at: self.clock.now(),
        })
    }

    /// «Vis meg hva appen ville gjort»: alle ventende forslag, uten beskyttede bilder, og med minst
    /// ett aktivt medlem igjen per gruppe (beste medlem beholdes). Ingen skriving.
    pub fn plan_pending(&self) -> Result<Plan> {
        let pending = self.store.pending_suggestions()?;
        let asset_ids: Vec<AssetId> = pending.iter().map(|p| p.asset_id).collect();
        let rows = self.store.asset_rows(&asset_ids)?;
        let mut picks: Vec<Selection> = Vec::new();
        let mut groups: Vec<crate::model::GroupId> = Vec::new();
        for (p, row) in pending.iter().zip(rows) {
            if row.is_some_and(|r| r.protect.is_protected()) {
                continue;
            }
            groups.extend(p.group);
            picks.push(Selection {
                id: p.asset_id,
                kind: Some(p.kind),
                reason: parse_reason(&p.reason_code),
            });
        }
        // Behold beste aktive medlem i hver gruppe som ellers ville blitt tom.
        let mut chosen: HashSet<AssetId> = picks.iter().map(|s| s.id).collect();
        groups.sort();
        groups.dedup();
        for (_, members) in self.store.active_members_of(&groups)? {
            if !members.is_empty() && members.iter().all(|m| chosen.contains(m)) {
                chosen.remove(&members[0]);
            }
        }
        picks.retain(|s| chosen.contains(&s.id));
        self.plan(&PlanRequest { items: picks })
    }

    /// I8: ingen berørt gruppe ender uten aktive medlemmer.
    fn guard_groups(&self, items: &[PlannedItem]) -> Result<()> {
        let chosen: HashSet<AssetId> = items.iter().map(|i| i.id).collect();
        let ids: Vec<AssetId> = items.iter().map(|i| i.id).collect();
        let mut groups: Vec<_> = self
            .store
            .groups_for(&ids)?
            .into_iter()
            .map(|(_, g)| g)
            .collect();
        groups.sort();
        groups.dedup();
        for (g, members) in self.store.active_members_of(&groups)? {
            if !members.is_empty() && members.iter().all(|m| chosen.contains(m)) {
                return Err(ActionError::WouldEmptyGroup(g));
            }
        }
        Ok(())
    }

    fn summarize(&self, items: &[PlannedItem], snap: &Snapshot) -> Result<ConsequenceSummary> {
        let (mut by_kind, mut prot) = (KindCounts::default(), ProtectCounts::default());
        let mut albums: BTreeMap<u64, (String, u64)> = BTreeMap::new();
        for it in items {
            match it.kind {
                Some(SuggestionKind::ExactDup) => by_kind.exact_dup += 1,
                Some(SuggestionKind::VisualDup) => by_kind.visual_dup += 1,
                Some(SuggestionKind::Blurry) => by_kind.blurry += 1,
                Some(SuggestionKind::Screenshot) => by_kind.screenshot += 1,
                Some(SuggestionKind::Series) => by_kind.series += 1,
                Some(SuggestionKind::Rule) => by_kind.rule += 1,
                None => by_kind.manual += 1,
            }
            prot.favorite += it.protect.has(ProtectFlags::FAVORITE) as u64;
            prot.shared_album += it.protect.has(ProtectFlags::SHARED_ALBUM) as u64;
            prot.edited += it.protect.has(ProtectFlags::EDITED) as u64;
        }
        let ids: Vec<AssetId> = items.iter().map(|i| i.id).collect();
        for (_, id, name) in self.store.albums_for(&ids)? {
            albums.entry(id).or_insert((name, 0)).1 += 1;
        }
        let mut albums: Vec<AlbumCount> = albums
            .into_iter()
            .map(|(album_id, (name, count))| AlbumCount {
                album_id,
                name,
                count,
            })
            .collect();
        albums.sort_by(|a, b| b.count.cmp(&a.count).then(a.album_id.cmp(&b.album_id)));

        let backup = if snap.rows.iter().any(|r| r.backup == Some(false)) {
            BackupSummary::Unverified
        } else if !snap.rows.is_empty() && snap.rows.iter().all(|r| r.backup == Some(true)) {
            BackupSummary::Verified
        } else {
            BackupSummary::Unknown
        };
        let count = items.len() as u64;
        let mut codes = Vec::new();
        for (flag, code) in ACK_CATEGORIES {
            if items.iter().any(|i| i.protect.has(flag)) {
                codes.push(code);
            }
        }
        if !albums.is_empty() {
            codes.push(ConsequenceCode::InAlbum);
        }
        match backup {
            BackupSummary::Unknown if count > 0 => codes.push(ConsequenceCode::BackupUnknown),
            BackupSummary::Unverified => codes.push(ConsequenceCode::NotBackedUp),
            _ => {}
        }
        if self.is_mass(count, snap.bytes) {
            codes.push(ConsequenceCode::MassDelete);
        }
        if count > 0 {
            codes.push(ConsequenceCode::FreesNoStorageYet);
        }
        Ok(ConsequenceSummary {
            count,
            bytes: snap.bytes,
            by_kind,
            albums,
            protected: prot,
            backup,
            // Vakten i `guard_groups` garanterer dette; feltet er for UI.
            groups_untouched_keeper: true,
            frees_storage_now: false,
            codes,
        })
    }

    pub(crate) fn is_mass(&self, count: u64, bytes: u64) -> bool {
        count >= self.cfg.mass_count || bytes >= self.cfg.mass_bytes
    }
}

#[allow(dead_code)]
/// Alle ack-er som trengs for planen (det `api` lager etter at brukeren har bekreftet hver kategori).
pub(crate) fn acks_covering(plan: &Plan) -> Acks {
    let mut protected = Vec::new();
    for (flag, _) in ACK_CATEGORIES {
        let ids: Vec<AssetId> = plan
            .items
            .iter()
            .filter(|i| i.protect.has(flag))
            .map(|i| i.id)
            .collect();
        if !ids.is_empty() {
            protected.push(ProtectedAck::new(flag, ids, plan.digest));
        }
    }
    Acks {
        protected,
        mass: Some(MassAck::new(plan.digest)),
    }
}
