//! Modul `protect` (L2). V1-gren `protect`: beskyttelse per bilde, mappe, album og person (alltid ack før sletting). Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt:
//! - `set(store, scope, now)` / `unset(store, scope) -> Option<Removal>` / `undo_unset(store, &Removal)`:
//!   sett og fjern beskyttelse. Å fjerne er konfigurasjon (ingen bilder berøres) og kan alltid angres.
//! - `list(store)`: alle beskyttelser. `effective(store, ids) -> Effective`: hvilke av bildene som er
//!   beskyttet og av hvilke omfang (bilde, mappe, album, person).
//! - `normalize(scope)`: kanonisk mål (mappeprefiks uten avsluttende skilletegn).
//!
//! `actions::plan` bruker `Store::protected_by_scope` (samme spørring) til `ConsequenceCode::ProtectedByScope`,
//! `ProtectCounts.scope` og til å holde beskyttede bilder utenfor `plan_pending`. Modulen sletter aldri noe.
//! Eier hele mappen `src/protect/` og `store/protection.rs`; ingen andre grener rører dem.

use crate::model::{AssetId, ProtectScope};
use crate::store::{Store, StoreError};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// Feil fra `protect`.
#[derive(Debug)]
pub enum ProtectError {
    /// Tom mappesti (ville truffet alt).
    EmptyFolder,
    /// Bilde, album eller person finnes ikke i indeksen.
    UnknownTarget(ProtectScope),
    Store(StoreError),
}

impl From<StoreError> for ProtectError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

pub type Result<T> = std::result::Result<T, ProtectError>;

/// Det som skal til for å angre `unset`: hele raden slik den var.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    pub scope: ProtectScope,
    pub created_at: i64,
}

/// Kanonisk form: mappesti trimmes og mister avsluttende `/` og `\`. Tom mappesti avvises.
pub fn normalize(scope: &ProtectScope) -> Result<ProtectScope> {
    match scope {
        ProtectScope::Folder(p) => {
            let t = p.trim().trim_end_matches(['/', '\\']);
            if t.is_empty() {
                Err(ProtectError::EmptyFolder)
            } else {
                Ok(ProtectScope::Folder(t.to_string()))
            }
        }
        other => Ok(other.clone()),
    }
}

/// Beskytter `scope`. `true` hvis ny, `false` hvis den allerede var satt (ingen endring).
pub fn set(store: &Store, scope: &ProtectScope, now: i64) -> Result<bool> {
    let scope = normalize(scope)?;
    if !store.protection_target_exists(&scope)? {
        return Err(ProtectError::UnknownTarget(scope));
    }
    Ok(store.protection_add(&scope, now)?)
}

/// Fjerner beskyttelsen. Gir `Removal` (til `undo_unset`), eller `None` hvis den ikke var satt.
pub fn unset(store: &Store, scope: &ProtectScope) -> Result<Option<Removal>> {
    let scope = normalize(scope)?;
    Ok(store
        .protection_remove(&scope)?
        .map(|(scope, created_at)| Removal { scope, created_at }))
}

/// Angrer `unset`: setter beskyttelsen tilbake med opprinnelig tidspunkt. `true` hvis den ble satt på nytt.
pub fn undo_unset(store: &Store, removal: &Removal) -> Result<bool> {
    Ok(store.protection_restore(&removal.scope, removal.created_at)?)
}

/// Alle beskyttelser (omfang, opprettet), eldste først.
pub fn list(store: &Store) -> Result<Vec<(ProtectScope, i64)>> {
    Ok(store.protection_list()?)
}

/// Effektiv beskyttelse for en mengde bilder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effective {
    by_asset: BTreeMap<AssetId, Vec<ProtectScope>>,
}

impl Effective {
    pub fn is_protected(&self, id: AssetId) -> bool {
        self.by_asset.contains_key(&id)
    }

    /// Omfangene som beskytter bildet (tom hvis ubeskyttet).
    pub fn scopes(&self, id: AssetId) -> &[ProtectScope] {
        self.by_asset.get(&id).map_or(&[], |v| v.as_slice())
    }

    /// Antall beskyttede bilder.
    pub fn count(&self) -> u64 {
        self.by_asset.len() as u64
    }

    /// Beskyttede bilder, sortert.
    pub fn ids(&self) -> Vec<AssetId> {
        self.by_asset.keys().copied().collect()
    }
}

/// Hvilke av `ids` som er beskyttet via omfang, og av hva.
pub fn effective(store: &Store, ids: &[AssetId]) -> Result<Effective> {
    let mut by_asset: BTreeMap<AssetId, Vec<ProtectScope>> = BTreeMap::new();
    for (id, scope) in store.protected_by_scope(ids)? {
        by_asset.entry(id).or_default().push(scope);
    }
    Ok(Effective { by_asset })
}
