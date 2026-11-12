//! `FsSource` med miniatyrbuffer: treff, ugyldiggjøring ved endret fil, buffer aldri i biblioteket,
//! plassholdere og avslått dekoding når aldri bufferen, og at originaler er urørte.

mod common;
use common::*;
use fjell_core::model::{AssetSource, SourceError, SourceId, Thumb, ThumbFmt};
use fjell_fs::thumbcache::ThumbCache;
use fjell_fs::FsSource;
use std::fs;
use std::path::Path;
use std::sync::Arc;

fn id(s: &str) -> SourceId {
    SourceId(s.into())
}

fn cache_files(dir: &Path) -> Vec<std::path::PathBuf> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "thumb"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn second_thumbnail_comes_from_cache() {
    let root = tempdir("tc-hit");
    let cdir = tempdir("tc-hit-cache");
    write(&root, "a.jpg", &jpeg(1000, 500, None));
    let src = FsSource::new(&root).with_thumb_cache(ThumbCache::new(&cdir, 1 << 24));
    let first = src.thumbnail(&id("a.jpg")).unwrap();
    assert_eq!((first.w, first.h), (256, 128));
    let files = cache_files(&cdir);
    assert_eq!(files.len(), 1, "én buffer-fil etter første kall");
    // Byttes bufferinnholdet, må neste kall gi det (bevis for treff uten ny dekoding).
    let marker = Thumb {
        w: 2,
        h: 2,
        fmt: ThumbFmt::Rgba8,
        data: vec![9; 16],
    };
    let mut raw = 2u32.to_le_bytes().to_vec();
    raw.extend_from_slice(&2u32.to_le_bytes());
    raw.extend_from_slice(&marker.data);
    fs::write(&files[0], raw).unwrap();
    assert_eq!(src.thumbnail(&id("a.jpg")).unwrap(), marker);
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&cdir);
}

#[test]
fn changed_file_gets_new_thumbnail() {
    let root = tempdir("tc-inv");
    let cdir = tempdir("tc-inv-cache");
    write(&root, "a.jpg", &jpeg(1000, 500, None));
    let src = FsSource::new(&root).with_thumb_cache(ThumbCache::new(&cdir, 1 << 24));
    let t1 = src.thumbnail(&id("a.jpg")).unwrap();
    assert_eq!((t1.w, t1.h), (256, 128));
    write(&root, "a.jpg", &jpeg(500, 1000, None)); // annen størrelse og innhold
    let t2 = src.thumbnail(&id("a.jpg")).unwrap();
    assert_eq!(
        (t2.w, t2.h),
        (128, 256),
        "ikke den gamle bufrede miniatyren"
    );
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&cdir);
}

#[test]
fn cache_inside_library_is_ignored_and_originals_untouched() {
    let root = tempdir("tc-in");
    write(&root, "a.jpg", &jpeg(300, 300, None));
    let before = snapshot(&root);
    let inside = root.join("cache");
    let src = FsSource::new(&root).with_thumb_cache(ThumbCache::new(&inside, 1 << 24));
    assert!(src.thumbnail(&id("a.jpg")).is_ok());
    assert!(!inside.exists(), "ingenting skrives i biblioteket");
    assert_eq!(snapshot(&root), before);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn originals_unchanged_with_cache_outside() {
    let root = tempdir("tc-orig");
    let cdir = tempdir("tc-orig-cache");
    write(&root, "a.jpg", &jpeg(600, 400, Some(6)));
    write(&root, "sub/b.png", b"ikke et bilde"); // ødelagt: skal gi Corrupt, ikke buffer
    let before = snapshot(&root);
    let src = FsSource::new(&root).with_thumb_cache(ThumbCache::new(&cdir, 1 << 24));
    assert!(src.thumbnail(&id("a.jpg")).is_ok());
    assert_eq!(
        src.thumbnail(&id("sub/b.png")).unwrap_err(),
        SourceError::Corrupt
    );
    assert_eq!(cache_files(&cdir).len(), 1, "feil bufres ikke");
    assert_eq!(snapshot(&root), before);
    // Alt under buffermappen er hex-navngitt `.thumb`.
    for p in cache_files(&cdir) {
        assert!(p.starts_with(&cdir));
    }
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&cdir);
}

#[test]
fn placeholder_and_no_decoding_never_touch_cache() {
    let root = tempdir("tc-ph");
    let cdir = tempdir("tc-ph-cache");
    write(&root, "cloud.jpg", &jpeg(50, 50, None));
    write(&root, "a.jpg", &jpeg(50, 50, None));
    let src = FsSource::new(&root)
        .with_locality(Arc::new(|p, _| !p.ends_with("cloud.jpg")))
        .with_thumb_cache(ThumbCache::new(&cdir, 1 << 24));
    assert_eq!(
        src.thumbnail(&id("cloud.jpg")).unwrap_err(),
        SourceError::Unavailable
    );
    assert!(cache_files(&cdir).is_empty());
    let off = FsSource::new(&root)
        .without_decoding()
        .with_thumb_cache(ThumbCache::new(&cdir, 1 << 24));
    assert_eq!(
        off.thumbnail(&id("a.jpg")).unwrap_err(),
        SourceError::Unavailable
    );
    assert!(cache_files(&cdir).is_empty());
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&cdir);
}

#[test]
fn cleared_cache_regenerates_and_path_escape_is_impossible() {
    let root = tempdir("tc-clr");
    let cdir = tempdir("tc-clr-cache");
    write(&root, "a.jpg", &jpeg(300, 300, None));
    let cache = ThumbCache::new(&cdir, 1 << 24);
    let src = FsSource::new(&root).with_thumb_cache(ThumbCache::new(&cdir, 1 << 24));
    let t = src.thumbnail(&id("a.jpg")).unwrap();
    cache.clear();
    assert!(cache_files(&cdir).is_empty());
    assert_eq!(src.thumbnail(&id("a.jpg")).unwrap(), t);
    // Id-er med `..` når aldri bufferen (avvises av `resolve`).
    assert!(src.thumbnail(&id("../x.jpg")).is_err());
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&cdir);
}
