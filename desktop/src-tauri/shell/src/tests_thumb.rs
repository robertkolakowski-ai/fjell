//! Tester for `Shell::thumbnail` mot temp-mapper (som `tests_core`).

use crate::dto::{DecisionDto, ThumbDto};
use crate::{production_source, Shell};
use base64::Engine;
use fjell_fs::{FakeRecycleBin, FsSource, RecycleBin};
use image::{DynamicImage, ImageFormat, RgbImage};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Env {
    root: PathBuf,
    lib: PathBuf,
    data: PathBuf,
    sh: Shell,
}

fn env(tag: &str) -> Env {
    let root = std::env::temp_dir().join(format!("fjell-thumb-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&data).unwrap();
    let bin = Arc::new(FakeRecycleBin {
        bin_dir: Some(root.join("papirkurv")),
        ..Default::default()
    });
    let sh = Shell::with_data_dir(&data, bin as Arc<dyn RecycleBin>);
    Env {
        root,
        lib,
        data,
        sh,
    }
}

impl Env {
    /// Åpner biblioteket med produksjonskilden (dekoding + miniatyrbuffer) og skanner.
    fn open_and_scan(&self, cloud: Option<&'static str>) {
        let mut src = production_source(&self.data, &self.lib);
        let probe: fjell_fs::locality::LocalProbe = Arc::new(move |p, _| {
            cloud.is_none_or(|n| p.file_name().and_then(|f| f.to_str()) != Some(n))
        });
        src = src.with_locality(probe.clone());
        self.sh
            .open_library_probe(&self.lib, Arc::new(src) as Arc<FsSource>, probe)
            .unwrap();
        self.sh.start_scan(true).unwrap();
        let t = Instant::now();
        while self.sh.state().scan_phase != "done" {
            assert!(
                t.elapsed() < Duration::from_secs(30),
                "skanning ble ikke ferdig"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn asset(&self, name: &str) -> u64 {
        let core = self.sh.core().unwrap();
        core.fjell.with_store(|s| {
            let conn = s.reader().unwrap().unwrap();
            conn.query_row("SELECT id FROM asset WHERE source_id = ?1", [name], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap() as u64
        })
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn image_bytes(w: u32, h: u32, seed: u32, fmt: ImageFormat) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x + seed) as u8, (y * 2 + seed) as u8, (x ^ y) as u8])
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(img)
        .write_to(&mut out, fmt)
        .unwrap();
    out.into_inner()
}

fn decode(t: &ThumbDto) -> image::DynamicImage {
    assert_eq!(t.mime, "image/jpeg");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&t.data)
        .unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "JPEG-signatur");
    image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg).unwrap()
}

#[test]
fn jpeg_gives_valid_jpeg_within_256_and_png_works() {
    let e = env("basic");
    fs::write(
        e.lib.join("a.jpg"),
        image_bytes(600, 400, 1, ImageFormat::Jpeg),
    )
    .unwrap();
    fs::write(
        e.lib.join("b.png"),
        image_bytes(300, 300, 2, ImageFormat::Png),
    )
    .unwrap();
    e.open_and_scan(None);
    let a = e.sh.thumbnail(e.asset("a.jpg")).unwrap();
    assert_eq!((a.width, a.height), (256, 171));
    let img = decode(&a);
    assert_eq!((img.width(), img.height()), (a.width, a.height));
    let b = e.sh.thumbnail(e.asset("b.png")).unwrap();
    assert_eq!((b.width, b.height), (256, 256));
    assert_eq!(decode(&b).width(), 256);
    // JSON-form: camelCase og uten data:-prefiks.
    let j = serde_json::to_value(&a).unwrap();
    assert!(j["width"].is_u64() && !j["data"].as_str().unwrap().starts_with("data:"));
}

#[test]
fn corrupt_missing_unknown_and_cloud_give_contract_codes() {
    let e = env("errors");
    fs::write(
        e.lib.join("ok.jpg"),
        image_bytes(40, 30, 3, ImageFormat::Jpeg),
    )
    .unwrap();
    fs::write(
        e.lib.join("bad.jpg"),
        b"\xFF\xD8\xFF not really a jpeg at all",
    )
    .unwrap();
    fs::write(
        e.lib.join("gone.jpg"),
        image_bytes(40, 30, 4, ImageFormat::Jpeg),
    )
    .unwrap();
    // Innholdet er søppel: hadde det blitt lest, ville svaret vært `corrupt`.
    fs::write(e.lib.join("cloud.jpg"), b"not an image").unwrap();
    e.open_and_scan(Some("cloud.jpg"));
    assert_eq!(
        e.sh.thumbnail(e.asset("bad.jpg")).unwrap_err().code,
        "corrupt"
    );
    assert_eq!(
        e.sh.thumbnail(e.asset("cloud.jpg")).unwrap_err().code,
        "cloud_placeholder"
    );
    let gone = e.asset("gone.jpg");
    fs::remove_file(e.lib.join("gone.jpg")).unwrap();
    assert_eq!(e.sh.thumbnail(gone).unwrap_err().code, "not_found");
    assert_eq!(e.sh.thumbnail(999_999).unwrap_err().code, "not_found");
    assert!(e.sh.thumbnail(e.asset("ok.jpg")).is_ok());
}

#[test]
fn no_library_and_mock_modes() {
    let e = env("modes");
    assert_eq!(e.sh.thumbnail(1).unwrap_err().code, "no_library");
    let m = Shell::mock().thumbnail(7).unwrap();
    assert_eq!(m, Shell::mock().thumbnail(7).unwrap(), "deterministisk");
    assert_eq!((decode(&m).width(), m.width), (m.width, 32));
}

#[test]
fn trashed_image_still_has_thumbnail_and_originals_are_untouched() {
    let e = env("trashed");
    let bytes = image_bytes(500, 300, 5, ImageFormat::Jpeg);
    fs::write(e.lib.join("a.jpg"), &bytes).unwrap();
    fs::write(e.lib.join("b.jpg"), &bytes).unwrap();
    let mtime = |n: &str| fs::metadata(e.lib.join(n)).unwrap().modified().unwrap();
    let before = (mtime("a.jpg"), mtime("b.jpg"));
    e.open_and_scan(None);
    let cards = e.sh.cards("exact_dup", 50, None).unwrap();
    let ids: Vec<u64> = cards.cards.iter().map(|c| c.id).collect();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    let trashed = e.sh.trashed().unwrap();
    assert_eq!(trashed.len(), 1);
    let name = trashed[0].name.clone();
    assert!(
        !e.lib.join(&name).exists(),
        "filen er flyttet til karantene"
    );
    let t = e.sh.thumbnail(trashed[0].asset).unwrap();
    assert_eq!((t.width, t.height), (256, 154));
    // Den som ble liggende, og originalene, er uendret.
    let kept = if name == "a.jpg" { "b.jpg" } else { "a.jpg" };
    assert_eq!(fs::read(e.lib.join(kept)).unwrap(), bytes);
    assert_eq!(
        mtime(kept),
        if kept == "a.jpg" { before.0 } else { before.1 }
    );
    // Etter angre ligger filen tilbake med samme innhold, og miniatyr virker fortsatt.
    e.sh.undo().unwrap();
    assert_eq!(fs::read(e.lib.join(&name)).unwrap(), bytes);
    assert!(e.sh.thumbnail(e.asset(&name)).is_ok());
}

#[test]
fn fifty_thumbnails_do_not_block_trash_and_undo() {
    let e = env("fifty");
    let dup = image_bytes(64, 48, 200, ImageFormat::Jpeg);
    fs::write(e.lib.join("dup1.jpg"), &dup).unwrap();
    fs::write(e.lib.join("dup2.jpg"), &dup).unwrap();
    for i in 0..50u32 {
        let name = format!("img{i:02}.jpg");
        fs::write(
            e.lib.join(name),
            image_bytes(1600, 1200, i, ImageFormat::Jpeg),
        )
        .unwrap();
    }
    e.open_and_scan(None);
    let ids: Vec<u64> = (0..50)
        .map(|i| e.asset(&format!("img{i:02}.jpg")))
        .collect();
    let sh = &e.sh;
    let loader_ids = ids;
    let cards: Vec<u64> = sh
        .cards("exact_dup", 50, None)
        .unwrap()
        .cards
        .iter()
        .map(|c| c.id)
        .collect();
    let mut worst = Duration::ZERO;
    std::thread::scope(|sc| {
        sc.spawn(|| {
            for id in &loader_ids {
                sh.thumbnail(*id).unwrap();
            }
        });
        for _ in 0..3 {
            let t = Instant::now();
            sh.decide(&cards, DecisionDto::Trash).unwrap();
            sh.undo().unwrap();
            worst = worst.max(t.elapsed());
        }
    });
    assert!(
        worst < Duration::from_secs(1),
        "trash+angre tok {worst:?} under lasting"
    );
}
