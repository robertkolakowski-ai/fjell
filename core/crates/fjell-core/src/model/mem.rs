//! Testdobler i `model` (L0): `MemSource` (i minnet, kan injisere feil og endringer) og `FakeClock`.
//!
//! Avhenger av: `super` (typer og traits). Ingen filsystem. Bruker bare i tester, CLI og mock.
//! Grensesnitt:
//! - `MemSource::put/update/remove/inject_failure/clear_failure` + `impl AssetSource`.
//! - `FakeClock::new/set/advance` + `impl Clock`.
//!
//! Endringsstrøm: hver mutasjon øker en versjon; `ChangeToken` er versjonen som tekst.

use super::{AssetMeta, AssetSource, ChangeSet, ChangeToken, Clock, SourceError, SourceId, Thumb};
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, MutexGuard};

struct Item {
    meta: AssetMeta,
    thumb: Option<Thumb>,
    bytes: Vec<u8>,
    ver: u64,
}

#[derive(Default)]
struct Inner {
    version: u64,
    items: BTreeMap<SourceId, Item>,
    removed: Vec<(u64, SourceId)>,
    failures: HashMap<SourceId, SourceError>,
}

/// Kilde i minnet.
#[derive(Default)]
pub struct MemSource {
    inner: Mutex<Inner>,
}

impl MemSource {
    pub fn new() -> MemSource {
        MemSource::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Legger til eller erstatter et element (teller som endring).
    pub fn put(&self, meta: AssetMeta, thumb: Option<Thumb>, bytes: Vec<u8>) {
        let mut g = self.lock();
        g.version += 1;
        let ver = g.version;
        g.removed.retain(|(_, s)| s != &meta.source_id);
        g.items.insert(
            meta.source_id.clone(),
            Item {
                meta,
                thumb,
                bytes,
                ver,
            },
        );
    }

    /// Endrer metadata (f.eks. ny `mod_token`). Returnerer false hvis ukjent.
    pub fn update(&self, meta: AssetMeta) -> bool {
        let mut g = self.lock();
        let next = g.version + 1;
        match g.items.get_mut(&meta.source_id) {
            Some(it) => {
                it.meta = meta;
                it.ver = next;
                g.version = next;
                true
            }
            None => false,
        }
    }

    /// Fjerner et element (meldes i `ChangeSet.removed`). Returnerer false hvis ukjent.
    pub fn remove(&self, id: &SourceId) -> bool {
        let mut g = self.lock();
        if g.items.remove(id).is_none() {
            return false;
        }
        g.version += 1;
        let v = g.version;
        g.removed.push((v, id.clone()));
        true
    }

    /// Alle kall for `id` (thumbnail og read_bytes) feiler med `err` til feilen fjernes.
    pub fn inject_failure(&self, id: &SourceId, err: SourceError) {
        self.lock().failures.insert(id.clone(), err);
    }

    pub fn clear_failure(&self, id: &SourceId) {
        self.lock().failures.remove(id);
    }

    pub fn len(&self) -> usize {
        self.lock().items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AssetSource for MemSource {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        let g = self.lock();
        let since_ver = match since {
            None => 0,
            Some(t) => {
                t.0.parse::<u64>()
                    .map_err(|_| SourceError::Failed("bad token".into()))?
            }
        };
        Ok(ChangeSet {
            changed: g
                .items
                .values()
                .filter(|i| i.ver > since_ver)
                .map(|i| i.meta.clone())
                .collect(),
            removed: g
                .removed
                .iter()
                .filter(|(v, _)| *v > since_ver)
                .map(|(_, s)| s.clone())
                .collect(),
            next_token: ChangeToken(g.version.to_string()),
        })
    }

    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        let g = self.lock();
        if let Some(e) = g.failures.get(id) {
            return Err(e.clone());
        }
        g.items
            .get(id)
            .ok_or(SourceError::NotFound)?
            .thumb
            .clone()
            .ok_or(SourceError::Unavailable)
    }

    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        let g = self.lock();
        if let Some(e) = g.failures.get(id) {
            return Err(e.clone());
        }
        let it = g.items.get(id).ok_or(SourceError::NotFound)?;
        Ok(Box::new(Cursor::new(it.bytes.clone())))
    }
}

/// Klokke som styres av testen.
#[derive(Default)]
pub struct FakeClock(AtomicI64);

impl FakeClock {
    pub fn new(now: i64) -> FakeClock {
        FakeClock(AtomicI64::new(now))
    }
    pub fn set(&self, now: i64) {
        self.0.store(now, Ordering::SeqCst);
    }
    pub fn advance(&self, secs: i64) {
        self.0.fetch_add(secs, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Kind, ProtectFlags, ThumbFmt};
    use super::*;

    pub(crate) fn meta(name: &str) -> AssetMeta {
        AssetMeta {
            source_id: SourceId(name.into()),
            kind: Kind::Photo,
            created_at: 1,
            mod_token: "a".into(),
            width: 4,
            height: 4,
            bytes: 3,
            mime: "image/jpeg".into(),
            platform_screenshot_hint: false,
            has_camera_exif: Some(true),
            is_local: true,
            protect_flags: ProtectFlags::default(),
            albums: vec![],
            backup: None,
        }
    }

    #[test]
    fn incremental_changes_and_removals() {
        let s = MemSource::new();
        s.put(meta("a"), None, vec![1]);
        s.put(meta("b"), None, vec![2]);
        let all = s.list_changes(None).unwrap();
        assert_eq!(all.changed.len(), 2);
        assert!(s
            .list_changes(Some(&all.next_token))
            .unwrap()
            .changed
            .is_empty());

        let mut m = meta("a");
        m.mod_token = "b".into();
        assert!(s.update(m));
        assert!(s.remove(&SourceId("b".into())));
        let d = s.list_changes(Some(&all.next_token)).unwrap();
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0].mod_token, "b");
        assert_eq!(d.removed, vec![SourceId("b".into())]);
        assert!(!s.remove(&SourceId("zz".into())));
    }

    #[test]
    fn reads_bytes_thumb_and_injected_failures() {
        let s = MemSource::new();
        let t = Thumb::new(1, 1, ThumbFmt::Gray8, vec![9]).unwrap();
        s.put(meta("a"), Some(t.clone()), vec![1, 2, 3]);
        s.put(meta("nothumb"), None, vec![]);
        let id = SourceId("a".into());
        assert_eq!(s.thumbnail(&id).unwrap(), t);
        let mut buf = Vec::new();
        s.read_bytes(&id).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, vec![1, 2, 3]);
        assert_eq!(
            s.thumbnail(&SourceId("nothumb".into())),
            Err(SourceError::Unavailable)
        );
        assert_eq!(
            s.thumbnail(&SourceId("zz".into())),
            Err(SourceError::NotFound)
        );

        s.inject_failure(&id, SourceError::Corrupt);
        assert_eq!(s.thumbnail(&id), Err(SourceError::Corrupt));
        assert!(matches!(s.read_bytes(&id), Err(SourceError::Corrupt)));
        s.clear_failure(&id);
        assert!(s.thumbnail(&id).is_ok());
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn bad_token_is_error_and_clock_moves() {
        let s = MemSource::new();
        assert!(s.list_changes(Some(&ChangeToken("x".into()))).is_err());
        let c = FakeClock::new(10);
        c.advance(5);
        assert_eq!(c.now(), 15);
        c.set(1);
        assert_eq!(c.now(), 1);
    }
}
