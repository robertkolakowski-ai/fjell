//! Fullflyt gjennom UniFFI-omslaget med en Rust-implementert «plattform» (MemSource):
//! skann → kort → trash → angre → trash → purge med billett. Verifiserer at omslaget bare delegerer.

use fjell_core::model::{self as m, AssetSource, MemSource};
use fjell_ffi::*;
use std::io::Read;
use std::sync::Arc;

struct Platform(Arc<MemSource>);

fn meta(id: &str) -> m::AssetMeta {
    m::AssetMeta {
        source_id: m::SourceId(id.into()),
        kind: m::Kind::Photo,
        created_at: 1_000,
        mod_token: "1".into(),
        width: 100,
        height: 100,
        bytes: 5,
        mime: "image/jpeg".into(),
        platform_screenshot_hint: false,
        has_camera_exif: Some(true),
        is_local: true,
        protect_flags: m::ProtectFlags(0),
        albums: vec![],
        backup: None,
    }
}

fn fail(e: m::SourceError) -> SourceFailure {
    match e {
        m::SourceError::Unavailable => SourceFailure::Unavailable,
        m::SourceError::Corrupt => SourceFailure::Corrupt,
        m::SourceError::NotFound => SourceFailure::NotFound,
        m::SourceError::Failed(d) => SourceFailure::Failed { detail: d },
    }
}

impl AssetSourceCallback for Platform {
    fn list_changes(&self, since: Option<String>) -> Result<ChangeSet, SourceFailure> {
        let tok = since.map(m::ChangeToken);
        let cs = self.0.list_changes(tok.as_ref()).map_err(fail)?;
        Ok(ChangeSet {
            changed: cs
                .changed
                .into_iter()
                .map(|a| AssetMeta {
                    source_id: a.source_id.0,
                    kind: AssetKind::Photo,
                    created_at: a.created_at,
                    mod_token: a.mod_token,
                    width: a.width,
                    height: a.height,
                    bytes: a.bytes,
                    mime: a.mime,
                    platform_screenshot_hint: false,
                    has_camera_exif: a.has_camera_exif,
                    is_local: true,
                    protect_flags: 0,
                    albums: vec![],
                    backup: None,
                })
                .collect(),
            removed: cs.removed.into_iter().map(|s| s.0).collect(),
            next_token: cs.next_token.0,
        })
    }
    fn thumbnail(&self, _id: String) -> Result<Thumb, SourceFailure> {
        Err(SourceFailure::Unavailable)
    }
    fn read_chunk(&self, id: String, offset: u64, max_len: u32) -> Result<Vec<u8>, SourceFailure> {
        let mut r = self.0.read_bytes(&m::SourceId(id)).map_err(fail)?;
        let mut all = Vec::new();
        r.read_to_end(&mut all).unwrap();
        let s = (offset as usize).min(all.len());
        let e = (s + max_len as usize).min(all.len());
        Ok(all[s..e].to_vec())
    }
}

fn cfg() -> ScanConfig {
    ScanConfig {
        mode: ScanMode::Quick,
        priority: 1,
        deadline: None,
    }
}

#[test]
fn scan_trash_undo_purge_through_wrapper() {
    let mem = Arc::new(MemSource::new());
    for n in ["a.jpg", "b.jpg", "c.jpg"] {
        mem.put(meta(n), None, b"same!".to_vec());
    }
    mem.put(meta("d.jpg"), None, b"other".to_vec());
    let dir = std::env::temp_dir().join(format!("fjell-ffi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("idx.sqlite").to_string_lossy().into_owned();
    let f = Fjell::open(db, Arc::new(Platform(mem.clone()))).unwrap();

    f.start_scan(cfg()).unwrap();
    f.wait_scan().unwrap();
    let page = f.cards(SuggestionKind::ExactDup, 10, None).unwrap();
    assert_eq!(page.cards.len(), 2, "tre like, én beholdes");
    let ds: Vec<_> = page
        .cards
        .iter()
        .map(|c| CardDecision {
            card: c.id,
            decision: Decision::Trash,
        })
        .collect();
    f.decide_many(ds.clone()).unwrap();
    assert_eq!(f.trash_summary().count, 2);
    assert_eq!(f.undo().unwrap().restored, 2);
    assert_eq!(f.trash_summary().count, 0);
    assert_eq!(f.undo().unwrap_err(), FjellError::NothingToUndo);

    let page = f.cards(SuggestionKind::ExactDup, 10, None).unwrap();
    let ds: Vec<_> = page
        .cards
        .iter()
        .map(|c| CardDecision {
            card: c.id,
            decision: Decision::Trash,
        })
        .collect();
    f.decide_many(ds).unwrap();
    let assets: Vec<u64> = page.cards.iter().map(|c| c.asset).collect();
    let intent = f.begin_purge(assets.clone()).unwrap();
    let ticket = match f.confirm_purge(intent.clone()) {
        Ok(t) => t,
        Err(FjellError::AckRequired { codes }) => f.confirm_purge_confirmed(intent, codes).unwrap(),
        Err(e) => panic!("{e:?}"),
    };
    let srcs = f.ticket_sources(ticket.clone());
    assert_eq!(srcs.len(), 2);
    let res = assets
        .iter()
        .map(|a| DeleteResult {
            asset: *a,
            outcome: DeleteOutcome::Deleted,
        })
        .collect();
    let rep = f.finish_purge(ticket, res).unwrap();
    assert_eq!(rep.purged.len(), 2);
    assert!(rep.kept_trashed.is_empty());
    assert_eq!(
        mem.len(),
        4,
        "kjernen sletter aldri selv: originalene er urørt"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn version_exposed() {
    assert_eq!(core_version(), VERSION);
}
