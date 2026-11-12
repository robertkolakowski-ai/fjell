//! Testoppsett for `pipeline` (K7): syntetisk korpus, telleverk rundt kilden og normalisering av
//! indeksen. Grensesnitt: `Fx` (kilde + indeks + klokke), `Counting` (teller thumbnail/read_bytes,
//! kan sette inn panikk og krok), `dump(&Fx)` (grupper og forslag uten id-er som kan variere).
//! Undermoduler: `scan` (K7-1…4, 7…9, 11), `robust` (K7-5, K7-6, K7-12, frist, avstemming).

mod migrate;
mod robust;
mod scan;

use super::*;
use crate::model::{
    AssetMeta, FakeClock, Kind, MemSource, ProtectFlags, SourceError, SourceId, Thumb, ThumbFmt,
};
use crate::store::Store;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Read;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

pub const W: u32 = 64;
pub const H: u32 = 48;

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Glatt, kantrik scene (sum av sinus og blokker). Ulike frø gir ulike hasher.
pub fn scene(seed: u64) -> Vec<u8> {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let waves: Vec<[f64; 4]> = (0..5)
        .map(|_| {
            [
                r.unit() * 0.5 + 0.05,
                r.unit() * 0.5 + 0.05,
                r.unit() * std::f64::consts::TAU,
                r.unit() * 40.0 + 15.0,
            ]
        })
        .collect();
    let blocks: Vec<[usize; 5]> = (0..4)
        .map(|_| {
            [
                (r.next() % 40) as usize,
                (r.next() % 30) as usize,
                10 + (r.next() % 18) as usize,
                8 + (r.next() % 14) as usize,
                (r.next() % 200) as usize + 20,
            ]
        })
        .collect();
    let mut px = vec![0u8; (W * H) as usize];
    for y in 0..H as usize {
        for x in 0..W as usize {
            let mut v = 120.0;
            for w in &waves {
                v += w[3] * (w[0] * x as f64 + w[1] * y as f64 + w[2]).sin();
            }
            for b in &blocks {
                if x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3] {
                    v = (v + b[4] as f64) / 2.0;
                }
            }
            px[y * W as usize + x] = v.clamp(0.0, 255.0) as u8;
        }
    }
    px
}

pub fn thumb(px: Vec<u8>) -> Thumb {
    Thumb::new(W, H, ThumbFmt::Gray8, px).unwrap()
}

/// Lett støy og litt lysere: samme motiv, annen fil.
pub fn noisy(px: &[u8], seed: u64) -> Vec<u8> {
    let mut r = Rng(seed | 1);
    px.iter()
        .map(|&p| (p as i32 + 4 + (r.next() % 5) as i32 - 2).clamp(0, 255) as u8)
        .collect()
}

/// Uskarp: 20 runder med 5×5 boksfilter i flyttall (kantene klemmes), kontraststrukket og kvantisert først til slutt.
pub fn blurred(px: &[u8]) -> Vec<u8> {
    let (w, h) = (W as i32, H as i32);
    let mut cur: Vec<f32> = px.iter().map(|&p| p as f32).collect();
    for _ in 0..20 {
        let mut next = cur.clone();
        for y in 0..h {
            for x in 0..w {
                let mut s = 0f32;
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let (xx, yy) = ((x + dx).clamp(0, w - 1), (y + dy).clamp(0, h - 1));
                        s += cur[(yy * w + xx) as usize];
                    }
                }
                next[(y * w + x) as usize] = s / 25.0;
            }
        }
        cur = next;
    }
    // Strekker kontrasten, ellers drukner glattheten i kvantiseringsstøy.
    let (lo, hi) = cur
        .iter()
        .fold((255f32, 0f32), |(l, h), v| (l.min(*v), h.max(*v)));
    cur.iter()
        .map(|v| ((v - lo) / (hi - lo).max(1.0) * 255.0).round() as u8)
        .collect()
}

pub fn meta(name: &str, bytes: u64, token: &str) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(name.into()),
        kind: Kind::Photo,
        created_at: 1_000 + name.len() as i64,
        mod_token: token.into(),
        width: 4032,
        height: 3024,
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

type Hook = Box<dyn Fn(usize) + Send + Sync>;

/// Kilde som teller kall og kan sette inn panikk og krok (kalles med nummeret på thumbnail-kallet).
pub struct Counting {
    pub inner: MemSource,
    pub thumbs: AtomicUsize,
    pub reads: AtomicUsize,
    pub panic_on: Mutex<HashSet<SourceId>>,
    pub hook: Mutex<Option<Arc<Hook>>>,
}

impl Counting {
    pub fn new() -> Counting {
        Counting {
            inner: MemSource::new(),
            thumbs: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            panic_on: Mutex::new(HashSet::new()),
            hook: Mutex::new(None),
        }
    }
    pub fn counts(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::SeqCst;
        (self.thumbs.load(SeqCst), self.reads.load(SeqCst))
    }
}

impl AssetSource for Counting {
    fn list_changes(
        &self,
        since: Option<&ChangeToken>,
    ) -> Result<crate::model::ChangeSet, SourceError> {
        self.inner.list_changes(since)
    }
    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        use std::sync::atomic::Ordering::SeqCst;
        let n = self.thumbs.fetch_add(1, SeqCst);
        let hook = self.hook.lock().unwrap().clone();
        if let Some(h) = hook {
            h(n);
        }
        if self.panic_on.lock().unwrap().contains(id) {
            panic!("injisert panikk");
        }
        self.inner.thumbnail(id)
    }
    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.read_bytes(id)
    }
}

pub struct Fx {
    pub src: Arc<Counting>,
    pub store: Arc<Mutex<Store>>,
    pub clock: FakeClock,
    pub cancel: Arc<AtomicBool>,
    /// Faste bilder: navn -> (miniatyr-piksler, bytes).
    pub files: BTreeMap<String, (Vec<u8>, Vec<u8>)>,
}

impl Fx {
    pub fn new() -> Fx {
        Fx::with_store(Store::open_in_memory().unwrap())
    }
    pub fn with_store(store: Store) -> Fx {
        Fx {
            src: Arc::new(Counting::new()),
            store: Arc::new(Mutex::new(store)),
            clock: FakeClock::new(2_000_000),
            cancel: Arc::new(AtomicBool::new(false)),
            files: BTreeMap::new(),
        }
    }

    /// Legger en fil i kilden. Filen er unik i størrelse hvis `bytes.len()` er det.
    pub fn put(&mut self, name: &str, px: Vec<u8>, bytes: Vec<u8>, token: &str) {
        let m = meta(name, bytes.len() as u64, token);
        self.src
            .inner
            .put(m, Some(thumb(px.clone())), bytes.clone());
        self.files.insert(name.into(), (px, bytes));
    }

    /// Fil med unik størrelse (hashes aldri av størrelsesforfilteret).
    pub fn uniq(&mut self, name: &str, px: Vec<u8>) {
        let n = self.files.len() + 1;
        self.put(name, px, vec![(n % 251) as u8; 1_000 + n * 7], "t1");
    }

    /// 28 bilder: 4 eksakte par, 3 visuelle duplikater, 2 uskarpe, 1 skjermbilde, 1 flatt, 10 unike.
    pub fn corpus(&mut self) {
        for i in 0..10u64 {
            self.uniq(&format!("u{i:02}.jpg"), scene(100 + i));
        }
        for i in 0..4u64 {
            let px = scene(200 + i);
            let bytes = vec![200 + i as u8; 5_000 + i as usize * 11];
            self.put(&format!("e{i}a.jpg"), px.clone(), bytes.clone(), "t1");
            self.put(&format!("e{i}b.jpg"), px, bytes, "t1");
        }
        for i in 0..3u64 {
            let px = scene(300 + i);
            self.uniq(&format!("v{i}a.jpg"), px.clone());
            self.uniq(&format!("v{i}b.jpg"), noisy(&px, i));
        }
        for i in 0..2u64 {
            self.uniq(&format!("b{i}.jpg"), blurred(&scene(400 + i)));
        }
        self.uniq("flat.jpg", vec![90; (W * H) as usize]);
        // Skjermbilde: plattformflagg + kjent skjermstørrelse + png.
        let mut m = meta("shot.png", 9_999, "t1");
        m.platform_screenshot_hint = true;
        m.has_camera_exif = Some(false);
        m.mime = "image/png".into();
        m.width = 1179;
        m.height = 2556;
        let px = scene(500);
        self.src
            .inner
            .put(m, Some(thumb(px.clone())), vec![5; 9_999]);
        self.files.insert("shot.png".into(), (px, vec![5; 9_999]));
    }

    pub fn scan(&self, cfg: &ScanConfig) -> ScanReport {
        let p = Progress::new();
        scan(
            self.src.as_ref(),
            self.store.as_ref(),
            &self.clock,
            cfg,
            &p,
            &self.cancel,
        )
        .unwrap()
    }

    pub fn scan_algo(&self, algo: u32) -> ScanReport {
        let p = Progress::new();
        scan_with(
            self.src.as_ref(),
            self.store.as_ref(),
            &self.clock,
            &ScanConfig::full(),
            &p,
            &self.cancel,
            crate::model::AlgoVer(algo),
        )
        .unwrap()
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        self.store.with_store(f)
    }

    pub fn id(&self, name: &str) -> crate::model::AssetId {
        let n = name.to_string();
        self.with(|s| {
            let v: i64 = s
                .conn()
                .query_row("SELECT id FROM asset WHERE source_id = ?1", [&n], |r| {
                    r.get(0)
                })
                .unwrap();
            crate::model::AssetId(v as u64)
        })
    }
}

/// Normalisert indeks: (grupper som (kind, keeper-navn, medlemsnavn)), forslag (navn, kind, grunn, konf-bits, status).
pub type Dump = (
    BTreeSet<(String, String, Vec<String>)>,
    BTreeSet<(String, String, String, u32, String)>,
);

pub fn dump(fx: &Fx) -> Dump {
    fx.with(|s| {
        let c = s.conn();
        let name = |id: i64| -> String {
            c.query_row("SELECT source_id FROM asset WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
        };
        let mut groups = BTreeSet::new();
        let mut st = c.prepare("SELECT id, kind, keeper_id FROM grp").unwrap();
        let gs: Vec<(i64, String, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for (g, kind, keeper) in gs {
            let mut ms: Vec<String> = c
                .prepare("SELECT asset_id FROM grp_member WHERE grp_id = ?1")
                .unwrap()
                .query_map([g], |r| r.get::<_, i64>(0))
                .unwrap()
                .map(|r| name(r.unwrap()))
                .collect();
            ms.sort();
            groups.insert((kind, name(keeper), ms));
        }
        let sugg = c
            .prepare("SELECT asset_id, kind, reason_code, confidence, status FROM suggestion")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .unwrap()
            .map(|r| {
                let (a, k, reason, conf, status) = r.unwrap();
                (name(a), k, reason, (conf as f32).to_bits(), status)
            })
            .collect();
        (groups, sugg)
    })
}
