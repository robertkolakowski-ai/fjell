//! Tester for K1 (K1-1, K1-2, K1-4…K1-6, K1-8) og K2a (K2a-1 lokalt, K2a-5, K2a-6).
//! Golden-delene (K1-3, K2a-2, K2a-4) ligger i `core/testdata/tests/golden_algos.rs`.

use super::*;
use crate::model::{AssetMeta, Kind, MemSource, ProtectFlags};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Enkel deterministisk RNG (xorshift64*) i stedet for proptest (ikke tillatt avhengighet ennå).
pub(crate) struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Leser som gir `total` bytes (mønster i % 251) i biter av høyst `chunk`.
struct Pattern {
    left: u64,
    pos: u64,
    chunk: usize,
}
impl Read for Pattern {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.chunk).min(self.left as usize);
        for b in &mut buf[..n] {
            *b = (self.pos % 251) as u8;
            self.pos += 1;
        }
        self.left -= n as u64;
        Ok(n)
    }
}

fn pattern(total: u64, chunk: usize) -> Pattern {
    Pattern {
        left: total,
        pos: 0,
        chunk,
    }
}

fn hex(h: [u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

fn oracle(total: u64) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    let mut p = pattern(total, 1 << 20);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = p.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    *h.finalize().as_bytes()
}

#[test]
fn k1_1_blake3_known_vectors() {
    // Offisielle BLAKE3-testvektorer (tom inndata og én byte 0x00).
    assert_eq!(
        hex(content_hash(&mut pattern(0, 8)).unwrap()),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    assert_eq!(
        hex(content_hash(&mut pattern(1, 8)).unwrap()),
        "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"
    );
    // 1 MiB og 100 MiB strømmes uten å holdes i minnet; referansen er blake3-crate med store biter.
    for total in [1u64 << 20, 100 << 20] {
        assert_eq!(
            content_hash(&mut pattern(total, 1 << 16)).unwrap(),
            oracle(total)
        );
    }
}

#[test]
fn k1_2_hash_independent_of_read_size() {
    let mut r = Rng(7);
    for _ in 0..20 {
        let total = r.below(3 << 20);
        let want = oracle(total);
        for chunk in [1usize, 4096, 1 << 20] {
            if chunk == 1 && total > 50_000 {
                continue; // 1 B-lesing: kortere inndata holder for å dekke grensen
            }
            assert_eq!(
                content_hash(&mut pattern(total, chunk)).unwrap(),
                want,
                "{total}/{chunk}"
            );
        }
    }
}

fn meta(name: &str, bytes: u64) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(name.into()),
        kind: Kind::Photo,
        created_at: 1,
        mod_token: "a".into(),
        width: 4,
        height: 4,
        bytes,
        mime: "image/jpeg".into(),
        platform_screenshot_hint: false,
        has_camera_exif: Some(true),
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    }
}

fn cand(id: u64, name: &str, bytes: u64, hashed: bool) -> HashCandidate {
    HashCandidate {
        id: AssetId(id),
        source: SourceId(name.into()),
        bytes,
        hashed,
    }
}

/// Teller `read_bytes`-kall per kilde.
struct Counting {
    inner: MemSource,
    reads: AtomicUsize,
}
impl AssetSource for Counting {
    fn list_changes(
        &self,
        s: Option<&crate::model::ChangeToken>,
    ) -> Result<crate::model::ChangeSet, SourceError> {
        self.inner.list_changes(s)
    }
    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        self.inner.thumbnail(id)
    }
    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read_bytes(id)
    }
}

fn counting(files: &[(&str, &[u8])]) -> Counting {
    let inner = MemSource::new();
    for (n, b) in files {
        inner.put(meta(n, b.len() as u64), None, b.to_vec());
    }
    Counting {
        inner,
        reads: AtomicUsize::new(0),
    }
}

#[test]
fn k1_4_unique_size_is_never_read() {
    let src = counting(&[("a", b"aaaa"), ("b", b"bbbbbb"), ("c", b"cc")]);
    let items = [
        cand(1, "a", 4, false),
        cand(2, "b", 6, false),
        cand(3, "c", 2, false),
    ];
    assert!(needs_hash(&items).is_empty());
    assert!(hash_pending(&src, &items).is_empty());
    assert_eq!(src.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn k1_5_new_file_with_same_size_hashes_both() {
    let src = counting(&[("a", b"aaaa"), ("b", b"bbbbbb")]);
    let mut items = vec![cand(1, "a", 4, false), cand(2, "b", 6, false)];
    assert!(needs_hash(&items).is_empty());
    // Ny fil med lik størrelse som den uhashede `a`.
    src.inner.put(meta("n", 4), None, b"aaaa".to_vec());
    items.push(cand(3, "n", 4, false));
    assert_eq!(needs_hash(&items), vec![AssetId(1), AssetId(3)]);
    let res = hash_pending(&src, &items);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1, res[1].1); // samme innhold, samme hash
    assert_eq!(src.reads.load(Ordering::SeqCst), 2);
    // Allerede hashet fil leses ikke på nytt, men utløser hash på uhashet partner.
    items[0].hashed = true;
    assert_eq!(needs_hash(&items), vec![AssetId(3)]);
}

#[test]
fn k1_6_same_size_different_content_gives_different_hashes() {
    let src = counting(&[("a", b"abcd"), ("b", b"abce")]);
    let items = [cand(1, "a", 4, false), cand(2, "b", 4, false)];
    let res = hash_pending(&src, &items);
    assert_eq!(res.len(), 2);
    assert_ne!(res[0].1.as_ref().unwrap(), res[1].1.as_ref().unwrap());
}

#[test]
fn k1_8_read_error_does_not_stop_others_and_is_retried() {
    let src = counting(&[("a", b"xxxx"), ("b", b"yyyy"), ("c", b"zzzz")]);
    src.inner
        .inject_failure(&SourceId("b".into()), SourceError::Unavailable);
    let items = [
        cand(1, "a", 4, false),
        cand(2, "b", 4, false),
        cand(3, "c", 4, false),
    ];
    let res = hash_pending(&src, &items);
    assert!(res[0].1.is_ok() && res[2].1.is_ok());
    assert_eq!(res[1].1, Err(SourceError::Unavailable));
    // Kalleren lagrer ikke noe for b, så den er fortsatt uhashet og kommer igjen neste skanning.
    let items = [
        cand(1, "a", 4, true),
        cand(2, "b", 4, false),
        cand(3, "c", 4, true),
    ];
    src.inner.clear_failure(&SourceId("b".into()));
    let res = hash_pending(&src, &items);
    assert_eq!((res.len(), res[0].0), (1, AssetId(2)));
    assert!(res[0].1.is_ok());
}

// ---------- K2a ----------

/// Syntetisk miniatyr uten bildefiler: glatte bølger pluss blokker, fra et frø.
pub(crate) fn synth(seed: u64, w: u32, h: u32) -> Thumb {
    let mut r = Rng(seed | 1);
    let (fx, fy, px, py) = (1 + r.below(5), 1 + r.below(5), r.below(64), r.below(64));
    let (bx, by) = (r.below(w as u64) as i64, r.below(h as u64) as i64);
    let mut data = Vec::new();
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let a = (x * fx as i64 * 255 / w as i64 + px as i64) % 128;
            let b = (y * fy as i64 * 255 / h as i64 + py as i64) % 128;
            let blk = if (x - bx).abs() < w as i64 / 6 && (y - by).abs() < h as i64 / 6 {
                90
            } else {
                0
            };
            data.push((a + b + blk).clamp(0, 255) as u8);
        }
    }
    Thumb::new(w, h, ThumbFmt::Gray8, data).unwrap()
}

fn to_rgba(t: &Thumb) -> Thumb {
    let data = t.data.iter().flat_map(|&v| [v, v, v, 255]).collect();
    Thumb::new(t.w, t.h, ThumbFmt::Rgba8, data).unwrap()
}

#[test]
fn k2a_5_gray_and_rgba_give_same_hash() {
    for (i, (w, h)) in [(256, 192), (200, 150), (97, 256), (5, 5)]
        .into_iter()
        .enumerate()
    {
        let g = synth(100 + i as u64, w, h);
        assert_eq!(perceptual(&g), perceptual(&to_rgba(&g)), "{w}x{h}");
        assert_eq!(low_info(&g), low_info(&to_rgba(&g)));
    }
}

#[test]
fn k2a_6_odd_sizes_do_not_panic() {
    let mut r = Rng(99);
    let mut sizes = vec![
        (1, 1),
        (256, 1),
        (1, 256),
        (200, 150),
        (2, 3),
        (255, 256),
        (3, 255),
    ];
    for _ in 0..60 {
        sizes.push((1 + r.below(256) as u32, 1 + r.below(256) as u32));
    }
    for (i, (w, h)) in sizes.into_iter().enumerate() {
        let t = synth(i as u64, w, h);
        let _ = perceptual(&t);
        let _ = low_info(&t);
        let _ = perceptual(&to_rgba(&t));
    }
    // Ugyldig miniatyr (feltene er offentlige): ingen panikk, behandles som lavinformasjon.
    let bad = Thumb {
        w: 10,
        h: 10,
        fmt: ThumbFmt::Gray8,
        data: vec![1; 3],
    };
    assert_eq!(perceptual(&bad), (Hash64(0), Hash64(0)));
    assert!(low_info(&bad));
    let zero = Thumb {
        w: 0,
        h: 0,
        fmt: ThumbFmt::Rgba8,
        data: vec![],
    };
    assert!(low_info(&zero));
}

#[test]
fn flat_images_are_low_info_and_textured_are_not() {
    let flat = Thumb::new(64, 64, ThumbFmt::Gray8, vec![128; 64 * 64]).unwrap();
    assert!(low_info(&flat));
    assert!(!low_info(&synth(1, 256, 192)));
}

#[test]
fn det_cos_matches_std_closely() {
    for m in 0..256usize {
        let want = (m as f64 * std::f64::consts::PI / 64.0).cos();
        assert!((det_cos(m) - want).abs() < 1e-12, "m={m}");
    }
}

#[test]
fn small_perturbation_changes_few_bits() {
    let a = synth(5, 256, 192);
    let mut b = a.clone();
    for v in b.data.iter_mut().step_by(7) {
        *v = v.saturating_add(3);
    }
    let ((da, pa), (db, pb)) = (perceptual(&a), perceptual(&b));
    assert!(da.hamming(db) <= 6 && pa.hamming(pb) <= 6);
    let (dc, _) = perceptual(&synth(6, 256, 192));
    assert!(da.hamming(dc) > 6);
}

/// Låser algoritmen (K2a-1, vektorfil): faste syntetiske miniatyrer med forventede hasher.
/// Samme konstanter må gi lik hash på alle plattformer (CI-matrisen kjører denne testen).
#[test]
fn k2a_1_hash_vectors() {
    const VECTORS: [(u64, u32, u32, u64, u64); 4] = [
        (11, 256, 192, 0x494949494f4f4f49, 0xcaca35ca92cf3531),
        (12, 192, 256, 0x7777777777777474, 0xad52ad5652ad5255),
        (13, 200, 150, 0x0101010101010101, 0x49b6b64949b7494d),
        (14, 256, 1, 0x5454545454545454, 0x9afb5591dc52140a),
    ];
    for (seed, w, h, d, p) in VECTORS {
        let (hd, hp) = perceptual(&synth(seed, w, h));
        assert_eq!(
            (hd.0, hp.0),
            (d, p),
            "algoritme endret: øk ALGO_VER (seed {seed}, {w}x{h}: {:#018x}, {:#018x})",
            hd.0,
            hp.0
        );
    }
}

#[test]
fn perceptual_cost_is_small() {
    // Kjernemål ≤ 2 ms per miniatyr (release); i debugbygg gir vi 10x slingringsmonn.
    let t = synth(77, 256, 192);
    let start = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(perceptual(&t));
    }
    let per = start.elapsed().as_secs_f64() / 20.0;
    let limit = if cfg!(debug_assertions) { 0.020 } else { 0.002 };
    assert!(per < limit, "perceptual {per:.4} s per miniatyr");
}
