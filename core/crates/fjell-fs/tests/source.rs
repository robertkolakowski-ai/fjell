//! `FsSource`: miniatyr (256 px, EXIF-orientering brukt), dimensjoner, skyplassholdere, frakoblet rot
//! (T27), lange stier og at originaler aldri endres.

mod common;
use common::*;
use fjell_core::model::{AssetSource, SourceError, SourceId, ThumbFmt};
use fjell_fs::FsSource;
use std::fs;
use std::sync::Arc;

fn id(s: &str) -> SourceId {
    SourceId(s.into())
}

#[test]
fn thumbnail_is_256_and_keeps_aspect() {
    let root = tempdir("thumb256");
    write(&root, "a.jpg", &jpeg(1000, 500, None));
    let t = FsSource::new(&root).thumbnail(&id("a.jpg")).unwrap();
    assert_eq!((t.w, t.h, t.fmt), (256, 128, ThumbFmt::Rgba8));
    assert_eq!(t.data.len(), 256 * 128 * 4);
    // Små bilder oppskaleres ikke.
    write(&root, "s.jpg", &jpeg(40, 20, None));
    let t = FsSource::new(&root).thumbnail(&id("s.jpg")).unwrap();
    assert_eq!((t.w, t.h), (40, 20));
}

#[test]
fn exif_orientation_applied_to_thumb_and_dims() {
    let root = tempdir("exif");
    // 6 = roter 90 med klokka: bredde/høyde byttes, venstre (rød) halvdel havner øverst.
    write(&root, "r6.jpg", &jpeg(40, 20, Some(6)));
    let src = FsSource::new(&root);
    let t = src.thumbnail(&id("r6.jpg")).unwrap();
    assert_eq!((t.w, t.h), (20, 40));
    let px = |x: usize, y: usize| &t.data[(y * 20 + x) * 4..(y * 20 + x) * 4 + 3];
    assert!(
        px(10, 2)[0] > 200 && px(10, 2)[2] < 60,
        "øverst skal være rød"
    );
    assert!(
        px(10, 37)[2] > 200 && px(10, 37)[0] < 60,
        "nederst skal være blå"
    );
    let m = &src.list_changes(None).unwrap().changed[0];
    assert_eq!((m.width, m.height), (20, 40), "meta er etter orientering");
    // Uten orientering: uendret.
    write(&root, "r1.jpg", &jpeg(40, 20, Some(1)));
    let t = src.thumbnail(&id("r1.jpg")).unwrap();
    assert_eq!((t.w, t.h), (40, 20));
}

#[test]
fn corrupt_vs_unavailable() {
    let root = tempdir("corrupt");
    write(&root, "bad.jpg", b"not a jpeg at all");
    write(&root, "clip.mp4", b"video bytes");
    let src = FsSource::new(&root);
    assert_eq!(src.thumbnail(&id("bad.jpg")), Err(SourceError::Corrupt));
    assert_eq!(
        src.thumbnail(&id("clip.mp4")),
        Err(SourceError::Unavailable)
    );
    assert_eq!(src.thumbnail(&id("none.jpg")), Err(SourceError::NotFound));
    // Skanning feiler ikke av ødelagt fil: dimensjoner 0.
    let cs = src.list_changes(None).unwrap();
    assert_eq!(cs.changed.len(), 2);
    assert!(cs.changed.iter().all(|m| m.width == 0));
}

#[test]
fn cloud_placeholder_is_not_local_and_never_read() {
    let root = tempdir("cloud");
    write(&root, "here.jpg", &jpeg(20, 20, None));
    write(&root, "cloud.jpg", &jpeg(20, 20, None));
    let src = FsSource::new(&root).with_locality(Arc::new(|p, _| !p.ends_with("cloud.jpg")));
    let cs = src.list_changes(None).unwrap();
    let get = |n: &str| cs.changed.iter().find(|m| m.source_id.0 == n).unwrap();
    assert!(get("here.jpg").is_local && get("here.jpg").width == 20);
    assert!(!get("cloud.jpg").is_local);
    assert_eq!(get("cloud.jpg").width, 0, "plassholdere dekodes aldri");
    assert_eq!(
        src.thumbnail(&id("cloud.jpg")),
        Err(SourceError::Unavailable)
    );
    assert!(matches!(
        src.read_bytes(&id("cloud.jpg")),
        Err(SourceError::Unavailable)
    ));
    assert!(src.read_bytes(&id("here.jpg")).is_ok());
}

#[test]
fn offline_root_is_unavailable_never_empty() {
    // T27 (adapterdelen): rot som forsvinner gir Unavailable, ikke tom liste uten `removed`.
    let root = tempdir("offline");
    write(&root, "a.jpg", &jpeg(20, 20, None));
    let src = FsSource::new(&root);
    assert_eq!(src.list_changes(None).unwrap().changed.len(), 1);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        src.list_changes(None).unwrap_err(),
        SourceError::Unavailable
    );
    assert_eq!(src.thumbnail(&id("a.jpg")), Err(SourceError::Unavailable));
    assert!(matches!(
        src.read_bytes(&id("a.jpg")),
        Err(SourceError::Unavailable)
    ));
}

#[test]
fn quarantine_dir_is_not_scanned_and_deep_paths_work() {
    let root = tempdir("deep");
    write(&root, ".fjell-karantene/1/1.jpg", &jpeg(20, 20, None));
    // Dyp sti (> 240 tegn totalt) virker på Linux; på Windows går den via `\\?\`.
    let deep = (0..8).map(|_| "d".repeat(30)).collect::<Vec<_>>().join("/");
    write(&root, &format!("{deep}/x.jpg"), &jpeg(20, 20, None));
    let src = FsSource::new(&root);
    let cs = src.list_changes(None).unwrap();
    assert_eq!(cs.changed.len(), 1);
    assert!(cs.changed[0].source_id.0.ends_with("/x.jpg"));
    assert!(src.read_bytes(&cs.changed[0].source_id).is_ok());
}

#[test]
fn scan_thumb_read_never_modify_originals() {
    let root = tempdir("orig");
    write(&root, "a.jpg", &jpeg(300, 200, Some(6)));
    write(&root, "sub/b.png", &{
        let mut v = Vec::new();
        image::DynamicImage::new_rgb8(10, 10)
            .write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png)
            .unwrap();
        v
    });
    let before = snapshot(&root);
    let src = FsSource::new(&root);
    for m in src.list_changes(None).unwrap().changed {
        src.thumbnail(&m.source_id).unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut src.read_bytes(&m.source_id).unwrap(), &mut buf).unwrap();
    }
    assert_eq!(snapshot(&root), before);
}
