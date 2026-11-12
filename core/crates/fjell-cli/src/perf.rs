//! `fjell-cli perf`: ytelsesmåling av kjernen mot syntetiske bilder (PLAN-KJERNE «Ytelsesmål», M8).
//!
//! Grensesnitt: `SynthSource` (deterministisk, lat generering, ingen filer) og `run(&PerfOptions, out)`.
//! Målingene bruker ekte kjerne (`Fjell`) med filbasert WAL-indeks:
//! 1. full pipeline over `n` bilder (tid per bilde inkludert generering av syntetiske miniatyrer, så
//!    tallet er en øvre grense for kjerneberegningen), skanning uten endring og inkrementell skanning,
//! 2. `decide_many`/`undo`/`cards` p95 på en indeks med `rows` bilder (ferdig skannet, med grupper),
//! 3. de samme p95-tallene mens en (bremset) skanning pågår.
//!
//! Budsjettene gjelder `--release`. I debug-bygg skaleres de med 25 (som i kjernens egne perf-tester).
//! Ingen nettverk, ingen eksterne filer: alt genereres i minnet fra et frø.

use fjell_core::api::{ApiError, Card, Decision, Fjell, FjellApi, ScanConfig, ScanMode, ScanPhase};
use fjell_core::model::{
    AssetMeta, AssetSource, ChangeSet, ChangeToken, Clock, Kind, ProtectFlags, SourceError,
    SourceId, SuggestionKind, Thumb, ThumbFmt,
};
use fjell_core::store::Store;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TW: u32 = 256;
const TH: u32 = 171;

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Unique,
    ExactDup(usize),
    NearDup(usize),
    Blurry,
    Screenshot,
}

/// Klasse for bilde `i`. Indekser som er multipler av 10 er alltid unike («primære»), så kopier
/// peker aldri på en kopi. Blant øvrige (90 %): ca. 11 % eksakte, 11 % nesten like, 5 % uskarpe og
/// 3 % skjermbilder.
fn class_of(i: usize) -> Class {
    if i.is_multiple_of(10) {
        return Class::Unique;
    }
    let r = mix(i as u64 ^ 0xA5A5);
    let base = 10 * ((mix(i as u64) % (i as u64 / 10 + 1)) as usize);
    match r % 1000 {
        0..=110 => Class::ExactDup(base),
        111..=221 => Class::NearDup(base),
        222..=271 => Class::Blurry,
        272..=301 => Class::Screenshot,
        _ => Class::Unique,
    }
}

/// Deterministisk kilde. Miniatyrer og bytes lages først når de spørres om.
pub struct SynthSource {
    count: Mutex<usize>,
    version: AtomicU64,
    /// (versjon, indeks) for hver endring etter oppstart.
    log: Mutex<Vec<(u64, usize)>>,
    bumps: Mutex<BTreeMap<usize, u64>>,
    /// Nanosekunder brukt i miniatyrgenerering (summert over tråder).
    pub gen_ns: AtomicU64,
    pub gen_calls: AtomicU64,
    /// Forsinkelse per miniatyr (simulerer treg kilde, så en skanning varer lenge nok til å måles).
    pub slow: Duration,
}

fn index_of(id: &SourceId) -> Option<usize> {
    id.0.strip_prefix("img/")?
        .strip_suffix(".jpg")?
        .parse()
        .ok()
}

impl SynthSource {
    pub fn new(n: usize) -> SynthSource {
        SynthSource {
            count: Mutex::new(n),
            version: AtomicU64::new(0),
            log: Mutex::new(Vec::new()),
            bumps: Mutex::new(BTreeMap::new()),
            gen_ns: AtomicU64::new(0),
            gen_calls: AtomicU64::new(0),
            slow: Duration::ZERO,
        }
    }

    pub fn with_delay(mut self, d: Duration) -> SynthSource {
        self.slow = d;
        self
    }

    pub fn len(&self) -> usize {
        *self.count.lock().unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Legger til `k` nye bilder (meldes som endringer).
    pub fn grow(&self, k: usize) {
        let mut c = self.count.lock().unwrap();
        let v = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        let mut log = self.log.lock().unwrap();
        for i in *c..*c + k {
            log.push((v, i));
        }
        *c += k;
    }

    /// Endrer `mod_token` på de gitte bildene (meldes som endret).
    pub fn bump(&self, idx: &[usize]) {
        let v = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        let mut log = self.log.lock().unwrap();
        let mut b = self.bumps.lock().unwrap();
        for &i in idx {
            *b.entry(i).or_default() += 1;
            log.push((v, i));
        }
    }

    fn content_seed(i: usize) -> usize {
        match class_of(i) {
            Class::ExactDup(b) => b,
            _ => i,
        }
    }

    fn bytes_len(i: usize) -> usize {
        // Spredning over 40 000 lengder gir ca. 12 % størrelseskollisjon ved 10 000 bilder.
        1000 + (mix(Self::content_seed(i) as u64 ^ 0x77) % 40_000) as usize
    }

    fn meta(&self, i: usize) -> AssetMeta {
        let class = class_of(i);
        let bump = self.bumps.lock().unwrap().get(&i).copied().unwrap_or(0);
        AssetMeta {
            source_id: SourceId(format!("img/{i:07}.jpg")),
            kind: Kind::Photo,
            created_at: 1_600_000_000 + i as i64 * 60,
            mod_token: format!("v{bump}"),
            width: 4000,
            height: 2670,
            bytes: Self::bytes_len(i) as u64,
            mime: "image/jpeg".into(),
            platform_screenshot_hint: class == Class::Screenshot,
            has_camera_exif: Some(class != Class::Screenshot),
            is_local: true,
            protect_flags: ProtectFlags::default(),
            albums: Vec::new(),
            backup: None,
        }
    }

    /// Miniatyr for bilde `i`: grov 17×13-flate (bildets «motiv») pluss fin støy (skarphet).
    fn make_thumb(i: usize) -> Thumb {
        let class = class_of(i);
        let coarse_seed = match class {
            Class::ExactDup(b) | Class::NearDup(b) => b,
            _ => i,
        } as u64;
        let fine_seed = Self::content_seed(i) as u64;
        let (gw, gh) = (17usize, 13usize);
        let grid: Vec<i32> = (0..gw * gh)
            .map(|c| (mix(coarse_seed * 1009 + c as u64) % 256) as i32)
            .collect();
        let amp: i64 = if class == Class::Blurry { 0 } else { 24 };
        let mut data = Vec::with_capacity(TW as usize * TH as usize * 4);
        let mut rng = mix(fine_seed ^ 0x1234) | 1;
        for y in 0..TH as usize {
            let gy = y * (gh - 1) * 256 / (TH as usize - 1);
            let (y0, fy) = ((gy / 256).min(gh - 2), (gy % 256) as i32);
            for x in 0..TW as usize {
                let gx = x * (gw - 1) * 256 / (TW as usize - 1);
                let (x0, fx) = ((gx / 256).min(gw - 2), (gx % 256) as i32);
                let g = |xx: usize, yy: usize| grid[yy * gw + xx];
                let top = g(x0, y0) * (256 - fx) + g(x0 + 1, y0) * fx;
                let bot = g(x0, y0 + 1) * (256 - fx) + g(x0 + 1, y0 + 1) * fx;
                let mut v = (top as i64 * (256 - fy) as i64 + bot as i64 * fy as i64) >> 16;
                if amp > 0 {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    v += (rng % (2 * amp as u64 + 1)) as i64 - amp;
                }
                let v = v.clamp(0, 255) as u8;
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        Thumb {
            w: TW,
            h: TH,
            fmt: ThumbFmt::Rgba8,
            data,
        }
    }
}

impl AssetSource for SynthSource {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        let ver = self.version.load(Ordering::SeqCst);
        let idx: Vec<usize> = match since {
            None => (0..self.len()).collect(),
            Some(t) => {
                let s: u64 =
                    t.0.parse()
                        .map_err(|_| SourceError::Failed("bad token".into()))?;
                let log = self.log.lock().unwrap();
                log.iter()
                    .filter(|(v, _)| *v > s)
                    .map(|(_, i)| *i)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }
        };
        Ok(ChangeSet {
            changed: idx.into_iter().map(|i| self.meta(i)).collect(),
            removed: Vec::new(),
            next_token: ChangeToken(ver.to_string()),
        })
    }

    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        let i = index_of(id).ok_or(SourceError::NotFound)?;
        if !self.slow.is_zero() {
            std::thread::sleep(self.slow);
        }
        let t = Instant::now();
        let th = Self::make_thumb(i);
        self.gen_ns
            .fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        self.gen_calls.fetch_add(1, Ordering::Relaxed);
        Ok(th)
    }

    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        let i = index_of(id).ok_or(SourceError::NotFound)?;
        let seed = Self::content_seed(i) as u64;
        let n = Self::bytes_len(i);
        let mut v = Vec::with_capacity(n + 8);
        let mut k = 0u64;
        while v.len() < n {
            v.extend_from_slice(&mix(seed * 31 + k).to_le_bytes());
            k += 1;
        }
        v.truncate(n);
        Ok(Box::new(Cursor::new(v)))
    }
}

struct SysClock;
impl Clock for SysClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64)
    }
}

/// Parametre for `perf`.
#[derive(Clone, Debug)]
pub struct PerfOptions {
    /// Antall bilder i pipeline-målingen (standard 10 000).
    pub n: usize,
    /// Antall bilder i indeksen for p95-målingene (standard 50 000).
    pub rows: usize,
    /// Mappe for midlertidige indeksfiler (opprettes; filene ryddes).
    pub dir: std::path::PathBuf,
    /// Antall runder per p95-måling.
    pub rounds: usize,
}

impl PerfOptions {
    pub fn standard(dir: &Path) -> PerfOptions {
        PerfOptions {
            n: 10_000,
            rows: 50_000,
            dir: dir.to_path_buf(),
            rounds: 500,
        }
    }
}

/// Målte tall (for tester og dokumentasjon).
#[derive(Clone, Debug, Default)]
pub struct PerfResult {
    pub pipeline_ms_per_image: f64,
    pub pipeline_total_s: f64,
    pub gen_ms_per_image: f64,
    pub unchanged_scan_ms: f64,
    pub incremental_scan_ms: f64,
    pub decide_p95_ms: f64,
    pub undo_p95_ms: f64,
    pub cards_p95_ms: f64,
    pub refresh_p95_ms: f64,
    pub scan_decide_p95_ms: f64,
    pub scan_undo_p95_ms: f64,
    pub scan_samples: usize,
    pub failures: Vec<String>,
}

fn factor() -> f64 {
    if cfg!(debug_assertions) {
        25.0
    } else {
        1.0
    }
}

pub fn p95(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 * 0.95).ceil() as usize).clamp(1, v.len()) - 1]
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn clean(dir: &Path, name: &str) {
    if std::env::var("FJ_KEEP").is_ok() {
        return;
    }
    for ext in ["db", "db-wal", "db-shm"] {
        let _ = std::fs::remove_file(dir.join(format!("{name}.{ext}")));
    }
}

fn fresh_db(dir: &Path, name: &str) -> Result<Store, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    clean(dir, name);
    Store::open(&dir.join(format!("{name}.db"))).map_err(|e| format!("{e:?}"))
}

fn start(f: &Fjell, mode: ScanMode) -> Result<(), String> {
    f.start_scan(ScanConfig {
        mode,
        priority: 1,
        deadline: None,
    })
    .map_err(|e| format!("{e:?}"))
}

fn scan_and_wait(f: &Fjell, mode: ScanMode) -> Result<fjell_core::pipeline::ScanReport, String> {
    start(f, mode)?;
    f.wait_scan()
        .ok_or("ingen rapport")?
        .map_err(|e| format!("{e:?}"))
}

/// Én runde: hent kort, felle 1–20 av dem, mål `decide_many`, så `undo`. Gir (cards, decide, undo) i ms.
/// `None` hvis det ikke fantes kort å teste med.
fn action_round(f: &Fjell, k: usize) -> Option<(f64, f64, f64)> {
    const SIZES: [usize; 6] = [1, 1, 1, 5, 20, 1];
    let kind = SuggestionKind::ALL[k % SuggestionKind::ALL.len()];
    let t = Instant::now();
    let page = f.cards(kind, 60, None).ok()?;
    let cards_ms = ms(t);
    let cards: &[Card] = &page.cards;
    if cards.is_empty() {
        return None;
    }
    let n = SIZES[k % SIZES.len()].min(cards.len());
    let start = (k * 7) % (cards.len() - n + 1);
    let decisions: Vec<_> = cards[start..start + n]
        .iter()
        .map(|c| (c.id, Decision::Trash))
        .collect();
    let t = Instant::now();
    match f.decide_many(decisions) {
        Ok(_) => {}
        Err(ApiError::AckRequired(_)) | Err(_) => return None,
    }
    let decide_ms = ms(t);
    let t = Instant::now();
    f.undo().ok()?;
    Some((cards_ms, decide_ms, ms(t)))
}

/// Kjører alle målinger og skriver en linje per tall. Budsjettbrudd ligger i `failures`.
pub fn run(o: &PerfOptions, out: &mut dyn Write) -> Result<PerfResult, String> {
    let mut r = PerfResult::default();
    let fac = factor();
    let mut say = |s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    say(format!(
        "perf: n={} rows={} profil={} (budsjettfaktor {fac})",
        o.n,
        o.rows,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    ))?;

    // 1. Pipeline.
    let src = Arc::new(SynthSource::new(o.n));
    let f = Fjell::new(
        fresh_db(&o.dir, "pipeline")?,
        src.clone(),
        Arc::new(SysClock),
    );
    let t = Instant::now();
    let rep = scan_and_wait(&f, ScanMode::Full)?;
    let total = t.elapsed();
    r.pipeline_total_s = total.as_secs_f64();
    r.pipeline_ms_per_image = total.as_secs_f64() * 1000.0 / o.n as f64;
    let calls = src.gen_calls.load(Ordering::Relaxed).max(1);
    r.gen_ms_per_image = src.gen_ns.load(Ordering::Relaxed) as f64 / 1e6 / calls as f64;
    say(format!(
        "pipeline full, {} bilder: {:.1} s totalt, {:.3} ms/bilde veggklokke (inkl. syntetisk miniatyr, som alene koster {:.3} ms CPU/bilde; {} tråder)",
        o.n,
        r.pipeline_total_s,
        r.pipeline_ms_per_image,
        r.gen_ms_per_image,
        std::thread::available_parallelism().map_or(1, |n| n.get())
    ))?;
    say(format!(
        "  trinn: enumerer {:?}, rask {:?}, full {:?}, klynging {:?}; hashet {}, analysert {}, feil {}",
        rep.timings.enumerate,
        rep.timings.quick,
        rep.timings.full,
        rep.timings.cluster,
        rep.hashed,
        rep.analyzed,
        rep.failures.len()
    ))?;
    if rep.analyzed != o.n as u64 || !rep.failures.is_empty() {
        r.failures.push(format!(
            "pipeline analyserte {} av {} (feil {})",
            rep.analyzed,
            o.n,
            rep.failures.len()
        ));
    }
    if r.pipeline_ms_per_image > 10.0 * fac {
        r.failures.push(format!(
            "pipeline {:.3} ms/bilde > {} ms",
            r.pipeline_ms_per_image,
            10.0 * fac
        ));
    }
    if r.pipeline_total_s > 600.0 * fac {
        r.failures
            .push(format!("pipeline {:.0} s > 600 s", r.pipeline_total_s));
    }
    let t = Instant::now();
    scan_and_wait(&f, ScanMode::Full)?;
    r.unchanged_scan_ms = ms(t);
    let changed: Vec<usize> = (0..o.n).step_by((o.n / 100).max(1)).take(100).collect();
    src.bump(&changed);
    let t = Instant::now();
    let rep = scan_and_wait(&f, ScanMode::Full)?;
    r.incremental_scan_ms = ms(t);
    say(format!(
        "skanning uten endring: {:.0} ms (budsjett 2000 ms); inkrementell, {} endret av {}: {:.0} ms (budsjett 3000 ms), meldt endret {}",
        r.unchanged_scan_ms,
        changed.len(),
        o.n,
        r.incremental_scan_ms,
        rep.changed
    ))?;
    if r.unchanged_scan_ms > 2000.0 * fac {
        r.failures.push("skanning uten endring > 2 s".into());
    }
    if r.incremental_scan_ms > 3000.0 * fac {
        r.failures.push("inkrementell skanning > 3 s".into());
    }
    drop(f);
    clean(&o.dir, "pipeline");

    // 2. Handlinger på `rows` bilder.
    let src = Arc::new(SynthSource::new(o.rows));
    let f = Fjell::new(fresh_db(&o.dir, "rows")?, src.clone(), Arc::new(SysClock));
    let t = Instant::now();
    scan_and_wait(&f, ScanMode::Full)?;
    say(format!(
        "indeks med {} bilder bygget på {:.1} s",
        o.rows,
        t.elapsed().as_secs_f64()
    ))?;
    let (mut c, mut d, mut u) = (vec![], vec![], vec![]);
    for k in 0..o.rounds {
        if let Ok(ms) = std::env::var("FJ_SLEEP") {
            let _ = f.sync_groups();
            std::thread::sleep(Duration::from_millis(ms.parse().unwrap_or(0)));
        }
        if let Some((a, b, e)) = action_round(&f, k) {
            c.push(a);
            d.push(b);
            u.push(e);
        }
    }
    let rounds_ok = d.len();
    if std::env::var("FJ_PROF").is_ok() {
        let med = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        eprintln!(
            "median cards {:.2} decide {:.2} undo {:.2}",
            med(&mut c),
            med(&mut d),
            med(&mut u)
        );
    }
    r.cards_p95_ms = p95(&mut c);
    r.decide_p95_ms = p95(&mut d);
    r.undo_p95_ms = p95(&mut u);
    say(format!(
        "p95 på {} bilder ({} runder): cards {:.2} ms, decide_many {:.2} ms, undo {:.2} ms (budsjett 10 ms kjernekall)",
        o.rows, rounds_ok, r.cards_p95_ms, r.decide_p95_ms, r.undo_p95_ms
    ))?;
    if rounds_ok < o.rounds / 2 {
        r.failures
            .push(format!("bare {rounds_ok} av {} runder ga kort", o.rounds));
    }
    for (name, v) in [
        ("cards", r.cards_p95_ms),
        ("decide_many", r.decide_p95_ms),
        ("undo", r.undo_p95_ms),
    ] {
        if v > 10.0 * fac {
            r.failures
                .push(format!("{name} p95 {v:.2} ms > {} ms", 10.0 * fac));
        }
    }
    // Gruppeoppdateringen etter en handling går i bakgrunnen (utenfor handlingens vei); mål hvor lenge den tar.
    let mut refresh = Vec::new();
    for k in 0..20 {
        let _ = f.sync_groups();
        let kind = SuggestionKind::ALL[k % 2];
        if let Ok(p) = f.cards(kind, 5, None) {
            if let Some(c) = p.cards.first() {
                if f.decide_many(vec![(c.id, Decision::Trash)]).is_ok() {
                    let t = Instant::now();
                    let _ = f.sync_groups();
                    refresh.push(ms(t));
                    let _ = f.undo();
                }
            }
        }
    }
    r.refresh_p95_ms = p95(&mut refresh);
    say(format!(
        "gruppeoppdatering i bakgrunnen etter en handling ({} bilder): p95 {:.1} ms (ikke en del av handlingens tid)",
        o.rows, r.refresh_p95_ms
    ))?;
    drop(f);
    clean(&o.dir, "rows");

    // 3. Handlinger under pågående skann: indeksen er ferdig skannet, så vokser kilden og en
    // bremset skanning av de nye bildene kjører mens handlingene måles.
    let extra = (o.rows / 10).max(200);
    let src = Arc::new(SynthSource::new(o.rows).with_delay(Duration::from_micros(300)));
    let f = Fjell::new(fresh_db(&o.dir, "rows2")?, src.clone(), Arc::new(SysClock));
    scan_and_wait(&f, ScanMode::Full)?;
    src.grow(extra);
    start(&f, ScanMode::Full)?;
    // Vent til skanningen faktisk har startet (fasen er `Done` fra forrige skanning inntil den nullstilles).
    let t0 = Instant::now();
    while matches!(
        f.home_summary().scan.phase,
        ScanPhase::Done | ScanPhase::Idle
    ) && t0.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    let (mut d, mut u) = (vec![], vec![]);
    let mut k = 0usize;
    while k < o.rounds * 20 {
        let ph = f.home_summary().scan.phase;
        if matches!(ph, ScanPhase::Done | ScanPhase::Idle) {
            break;
        }
        if let Some((_, a, b)) = action_round(&f, k) {
            d.push(a);
            u.push(b);
        }
        k += 1;
    }
    let _ = f.wait_scan();
    r.scan_samples = d.len();
    r.scan_decide_p95_ms = p95(&mut d);
    r.scan_undo_p95_ms = p95(&mut u);
    say(format!(
        "p95 under pågående skann ({extra} nye bilder, {} runder): decide_many {:.2} ms, undo {:.2} ms (budsjett 10 ms)",
        r.scan_samples, r.scan_decide_p95_ms, r.scan_undo_p95_ms
    ))?;
    if r.scan_samples < 20 {
        r.failures.push(format!(
            "bare {} målinger under skann (skanningen var for kort)",
            r.scan_samples
        ));
    }
    for (name, v) in [
        ("decide_many", r.scan_decide_p95_ms),
        ("undo", r.scan_undo_p95_ms),
    ] {
        if v > 10.0 * fac {
            r.failures.push(format!(
                "{name} under skann p95 {v:.2} ms > {} ms",
                10.0 * fac
            ));
        }
    }
    drop(f);
    clean(&o.dir, "rows2");
    for f in &r.failures {
        say(format!("BRUDD: {f}"))?;
    }
    say(if r.failures.is_empty() {
        "perf: alle budsjetter holdt".to_string()
    } else {
        format!("perf: {} budsjettbrudd", r.failures.len())
    })?;
    Ok(r)
}
