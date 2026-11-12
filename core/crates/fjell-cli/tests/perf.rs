//! `fjell-cli perf`: at kommandoen kjører, måler og feiler ved budsjettbrudd (liten størrelse; de
//! reelle tallene måles med `just bench` i release, se docs/mvp/YTELSE.md). Syntetisk kilde er deterministisk.

use fjell_cli::perf::{p95, run, PerfOptions, SynthSource};
use fjell_core::model::AssetSource;

#[test]
fn synth_source_is_deterministic_and_has_duplicates() {
    let a = SynthSource::new(300);
    let b = SynthSource::new(300);
    let (ma, mb) = (a.list_changes(None).unwrap(), b.list_changes(None).unwrap());
    assert_eq!(ma.changed, mb.changed);
    let id = &ma.changed[7].source_id;
    assert_eq!(a.thumbnail(id).unwrap(), b.thumbnail(id).unwrap());
    // Minst ett par med identiske bytes.
    let mut seen = std::collections::BTreeMap::new();
    let mut dups = 0;
    for m in &ma.changed {
        let mut v = Vec::new();
        std::io::Read::read_to_end(&mut a.read_bytes(&m.source_id).unwrap(), &mut v).unwrap();
        if seen
            .insert(blake3::hash(&v).to_hex().to_string(), ())
            .is_some()
        {
            dups += 1;
        }
    }
    assert!(dups > 10, "{dups}");
}

#[test]
fn synth_source_reports_incremental_changes() {
    let s = SynthSource::new(100);
    let first = s.list_changes(None).unwrap();
    s.bump(&[3, 4]);
    s.grow(5);
    let inc = s.list_changes(Some(&first.next_token)).unwrap();
    assert_eq!(inc.changed.len(), 7);
    assert_eq!(
        inc.changed.iter().filter(|m| m.mod_token == "v1").count(),
        2
    );
}

#[test]
fn p95_picks_the_95th_percentile() {
    let mut v: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(p95(&mut v), 95.0);
    assert_eq!(p95(&mut []), 0.0);
}

#[test]
fn perf_run_small_completes_within_scaled_budgets() {
    let dir = std::env::temp_dir().join(format!("fjell-perfcli-{}", std::process::id()));
    let o = PerfOptions {
        n: 150,
        rows: 800,
        dir: dir.clone(),
        rounds: 40,
    };
    let mut out = Vec::new();
    let r = run(&o, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("pipeline full, 150 bilder"), "{text}");
    assert!(r.failures.is_empty(), "{text}");
    assert!(r.pipeline_ms_per_image > 0.0 && r.decide_p95_ms >= 0.0);
    let _ = std::fs::remove_dir_all(&dir);
}
