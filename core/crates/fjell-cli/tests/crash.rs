//! K7-6: krasj (prosessdrap) midt i skriving. Barneprosessen (`fjell-cli crash-child`) drepes med
//! SIGKILL (`Child::kill`), uten mulighet til å rydde. Så åpnes indeksen på nytt (WAL gjenopprettes):
//! integritet, ingen halv batch, og gjenopptak gir samme sluttresultat som en uavbrutt skanning.
//! Barnet er en ekte prosess; dette er ikke en simulering med `drop`.

use fjell_cli::crashtest::ACTION_BATCH;
use fjell_cli::perf::SynthSource;
use fjell_core::api::{Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{Clock, ReasonCode, SuggestionKind};
use fjell_core::store::Store;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

struct Zero;
impl Clock for Zero {
    fn now(&self) -> i64 {
        1_700_000_000
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fjell-crash-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Starter barnet, venter til `wait_lines` linjer er lest (skrivingen pågår), sover `extra`, og dreper med SIGKILL.
fn run_and_kill(
    mode: &str,
    db: &Path,
    n: usize,
    until: impl Fn(&[String]) -> bool,
    extra: Duration,
) -> Vec<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fjell-cli"))
        .args(["crash-child", mode, &db.to_string_lossy(), &n.to_string()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = Vec::new();
    let mut rd = BufReader::new(child.stdout.take().unwrap());
    while !until(&lines) {
        let mut l = String::new();
        if rd.read_line(&mut l).unwrap() == 0 {
            break;
        }
        lines.push(l.trim().to_string());
    }
    std::thread::sleep(extra);
    child.kill().unwrap(); // SIGKILL på Unix, TerminateProcess på Windows: ingen opprydding
    let status = child.wait().unwrap();
    assert!(!status.success(), "barnet skulle vært drept: {status:?}");
    lines
}

fn open(db: &Path, src: Arc<SynthSource>) -> Fjell {
    Fjell::new(Store::open(db).unwrap(), src, Arc::new(Zero))
}

fn scan(f: &Fjell) -> fjell_core::pipeline::ScanReport {
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap()
}

/// Siste `progress <n>`-linje fra barnet (0 hvis ingen).
fn progress(lines: &[String]) -> u64 {
    lines
        .iter()
        .filter_map(|l| l.strip_prefix("progress ")?.parse().ok())
        .max()
        .unwrap_or(0)
}

type Row = (u64, SuggestionKind, ReasonCode, u32);

/// Alle kort (asset, type, begrunnelse, konfidens i promille), sortert: sammenligningsgrunnlaget.
fn all_cards(f: &Fjell) -> Vec<Row> {
    let mut v = Vec::new();
    for k in SuggestionKind::ALL {
        let mut cur = None;
        loop {
            let p = f.cards(k, 500, cur).unwrap();
            v.extend(p.cards.iter().map(|c| {
                (
                    c.asset.0,
                    k,
                    c.reason,
                    (c.confidence * 1000.0).round() as u32,
                )
            }));
            match p.next {
                Some(n) => cur = Some(n),
                None => break,
            }
        }
    }
    v.sort_by_key(|r| (r.0, r.1.as_str().to_string()));
    v
}

/// Kjører barnet i `scan`-modus og dreper det på ulike tidspunkter; hver gang: konsistent indeks,
/// og gjenopptak gir samme kort som uavbrutt skanning.
#[test]
fn kill_dash_9_during_scan_leaves_no_partial_state_and_resume_completes() {
    const N: usize = 1500;
    let dir = tmp("scan");
    // Fasit: uavbrutt skanning.
    let reference = {
        let f = open(&dir.join("ref.db"), Arc::new(SynthSource::new(N)));
        scan(&f);
        f.sync_groups().unwrap();
        all_cards(&f)
    };
    assert!(reference.len() > 150, "fasiten må ha innhold");
    let mut killed_mid_write = false;
    for (round, target) in [1u64, 100, 600, 1200].into_iter().enumerate() {
        let db = dir.join(format!("c{round}.db"));
        let lines = run_and_kill("scan", &db, N, |l| progress(l) >= target, Duration::ZERO);
        assert_eq!(lines.first().map(String::as_str), Some("ready"));
        // 1. Åpning gjenoppretter WAL; ingen avvik.
        let store = Store::open(&db).unwrap();
        assert_eq!(
            store.verify().unwrap(),
            Vec::<String>::new(),
            "runde {round}"
        );
        drop(store);
        // 2. Gjenopptak (ny prosess-lignende åpning) fullfører og gir samme resultat som fasiten.
        let f = open(&db, Arc::new(SynthSource::new(N)));
        let resumed = scan(&f);
        eprintln!(
            "runde {round}: kill ved fremdrift {target}, gjenopptak analyserte {} av {N}",
            resumed.analyzed
        );
        killed_mid_write |= resumed.analyzed > 0 && resumed.analyzed < N as u64;
        f.sync_groups().unwrap();
        assert_eq!(
            all_cards(&f),
            reference,
            "runde {round}: resultat etter gjenopptak"
        );
        f.with_store(|s| assert_eq!(s.verify().unwrap(), Vec::<String>::new()));
        let again = {
            f.start_scan(ScanConfig {
                mode: ScanMode::Full,
                priority: 1,
                deadline: None,
            })
            .unwrap();
            f.wait_scan().unwrap().unwrap()
        };
        assert_eq!(
            again.analyzed, 0,
            "runde {round}: ingenting igjen å analysere"
        );
    }
    assert!(
        killed_mid_write,
        "ingen runde drepte barnet midt i full fase; juster tidspunktene"
    );
}

/// Drepper barnet midt i løkka `decide_many`/`undo`: hver batch i loggen er hel (20 rader eller
/// like mange som angret), og tilstanden stemmer med loggen.
#[test]
fn kill_dash_9_during_actions_leaves_whole_batches_only() {
    const N: usize = 3000;
    let dir = tmp("actions");
    let db = dir.join("a.db");
    {
        let f = open(&db, Arc::new(SynthSource::new(N)));
        scan(&f);
        f.sync_groups().unwrap();
    }
    for (round, delay_ms) in [0u64, 7, 40, 200].into_iter().enumerate() {
        // Vent til minst to batcher er ferdige (to linjer etter `ready`), så drep midt i neste.
        let lines = run_and_kill(
            "actions",
            &db,
            N,
            |l| l.len() >= 3,
            Duration::from_millis(delay_ms),
        );
        assert!(lines.len() >= 3, "barnet kom ikke i gang: {lines:?}");
        let store = Store::open(&db).unwrap();
        assert_eq!(
            store.verify().unwrap(),
            Vec::<String>::new(),
            "runde {round}"
        );
        for (batch, rows) in store.batch_sizes().unwrap() {
            assert_eq!(
                rows, ACTION_BATCH as u64,
                "runde {round}: batch {batch} har {rows} rader (halv batch)"
            );
        }
        // Angre-stakken virker etter krasjen: en eventuell gjenstående batch kan angres hel.
        drop(store);
        let f = open(&db, Arc::new(SynthSource::new(N)));
        let before = f.trash_summary().count;
        if before > 0 {
            let u = f.undo().unwrap();
            assert_eq!(u.restored, ACTION_BATCH as u64);
            assert_eq!(f.trash_summary().count, before - ACTION_BATCH as u64);
        }
        f.sync_groups().unwrap();
        f.with_store(|s| assert_eq!(s.verify().unwrap(), Vec::<String>::new()));
    }
}
