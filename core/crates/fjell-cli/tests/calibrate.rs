//! `fjell-cli calibrate` mot golden-korpuset fra `core/testdata` (generert av `fjell-gen-golden`,
//! etikettfilen lages her fra `manifest.json`). Originalene må være uendret etter kjøring.

use fjell_cli::calibrate::{evaluate, parse_labels, Options};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn corpus() -> &'static PathBuf {
    static C: OnceLock<PathBuf> = OnceLock::new();
    C.get_or_init(|| {
        // Rydd etter tidligere kjøringer (diskplass).
        for e in fs::read_dir(std::env::temp_dir())
            .into_iter()
            .flatten()
            .flatten()
        {
            if e.file_name().to_string_lossy().starts_with("fjell-calib-") {
                let _ = fs::remove_dir_all(e.path());
            }
        }
        let out = std::env::temp_dir().join(format!("fjell-calib-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/Cargo.toml");
        let st = Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
            .args(["run", "--quiet", "--locked", "--manifest-path"])
            .arg(manifest)
            .args(["--bin", "fjell-gen-golden", "--"])
            .arg(&out)
            .status()
            .expect("cargo run");
        assert!(st.success(), "generator feilet");
        out
    })
}

/// (fil, eksakt, visuell, uskarp) fra manifest.json (pen JSON, ett felt per linje).
fn manifest_rows(dir: &Path) -> Vec<(String, String, String, String)> {
    let text = fs::read_to_string(dir.join("manifest.json")).unwrap();
    let val = |l: &str| {
        l.split_once(": ")
            .unwrap()
            .1
            .trim_end_matches(',')
            .trim_matches('"')
            .to_string()
    };
    let (mut rows, mut cur) = (
        Vec::new(),
        (String::new(), String::new(), String::new(), String::new()),
    );
    for l in text.lines().map(str::trim) {
        if l.starts_with("\"file\":") {
            cur.0 = val(l);
        } else if l.starts_with("\"exact_group\":") {
            cur.1 = val(l).replace("null", "");
        } else if l.starts_with("\"visual_group\":") {
            cur.2 = val(l).replace("null", "");
        } else if l.starts_with("\"blurry\":") {
            cur.3 = match val(l).as_str() {
                "true" => "ja",
                "false" => "nei",
                _ => "",
            }
            .into();
            rows.push(std::mem::take(&mut cur));
        }
    }
    rows
}

fn labels_csv(dir: &Path, delim: char) -> String {
    let d = delim.to_string();
    let mut s = ["fil", "eksakt_gruppe", "visuell_gruppe", "uskarp"].join(&d);
    for (f, e, v, b) in manifest_rows(dir) {
        let pre = |x: String, p: &str| if x.is_empty() { x } else { format!("{p}{x}") };
        s += &format!("\n{}", [f, pre(e, "e"), pre(v, "v"), b].join(&d));
    }
    s
}

fn sums(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(d: &Path, m: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, m);
            } else {
                m.insert(p.clone(), fs::read(&p).unwrap());
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(&root.join("images"), &mut m);
    m
}

#[test]
fn calibrate_golden_corpus_matches_known_quality_and_leaves_originals() {
    let root = corpus();
    let images = root.join("images");
    let before = sums(root);
    let csv = root.join("labels.csv");
    // Etikettene gjelder bildene; stiene er relative til images/.
    let text = labels_csv(root, ',').replace("\nimages/", "\n");
    fs::write(&csv, &text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_fjell-cli"))
        .args(["calibrate", "--dir"])
        .arg(&images)
        .arg("--labels")
        .arg(&csv)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for k in [
        "ALGO_VER 1",
        "[eksakte duplikater]",
        "[visuell hash-gruppering]",
        "[falske sammenslåinger]",
        "[uskarphet]",
        "max_dist=6 max_span=10 tight_dist=4",
    ] {
        assert!(stdout.contains(k), "mangler {k}:\n{stdout}");
    }
    // Eksakte duplikater er deterministiske: full presisjon og recall.
    assert!(
        stdout.contains("eksakt: presisjon 1.000 recall 1.000"),
        "{stdout}"
    );
    assert_eq!(before, sums(root), "originaler endret");
}

#[test]
fn calibrate_metrics_in_process_and_semicolon_csv() {
    let root = corpus();
    let images = root.join("images");
    let text = labels_csv(root, ';')
        .replace("\nimages/", "\n")
        .replace('\n', "\r\n");
    let labels = parse_labels(&text).unwrap();
    let src = fjell_fs::FsSource::new(&images);
    let r = evaluate(&src, &labels, &Options::default()).unwrap();
    assert_eq!(r.unlabeled, 0);
    assert!(r.labeled > 300, "{}", r.labeled);
    let (a, b) = (r.visual_all.unwrap(), r.visual_reps.unwrap());
    // Alle golden-grupper regnet som fasit (også beskjæring og uskarpe varianter som hashene ikke
    // skal finne): presisjon 1,0, recall målt 0,72 (144 av 200 par). Terskelen under er en
    // låsing av dagens måling, ikke et akseptkriterium.
    assert_eq!(a.precision(), Some(1.0));
    assert!(a.recall().unwrap() >= 0.70, "{:?}", a);
    assert!(b.precision().unwrap() >= 0.98);
    assert_eq!(r.false_pair_total, 0);
    let bl = r.blur_app.unwrap();
    assert!(
        bl.precision().unwrap() >= 0.90 && bl.recall().unwrap() >= 0.90,
        "{bl:?}"
    );
    // Kandidatparametere kan overstyres uten at algoritmen endres.
    let mut o = Options::default();
    o.params.max_span = 12;
    o.params.tight_dist = 5;
    let r2 = evaluate(&src, &labels, &o).unwrap();
    assert!(r2.visual_all.unwrap().recall() >= a.recall());
}

#[test]
fn calibrate_rejects_bad_labels_and_reports_false_merges() {
    assert!(parse_labels("a,b\n1,2").is_err());
    assert!(parse_labels("fil\nx").is_err());
    assert!(parse_labels("fil,uskarp\nx,kanskje").is_err());
    assert!(parse_labels("fil,uskarp\nx,ja\nx,nei").is_err());
    let l = parse_labels("\u{feff}fil;uskarp\n\"a;b.png\";ja\n.\\c.png;\n").unwrap();
    assert_eq!(l.rows[0].file, "a;b.png");
    assert_eq!(l.rows[1].file, "c.png");
    assert_eq!(l.rows[1].blurry, None);

    // To ulike motiver merket som ulike grupper, men like for hash: falsk sammenslåing.
    let root = corpus();
    let rows = manifest_rows(root);
    let mut by_vis: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (f, _, v, _) in &rows {
        if !v.is_empty() {
            by_vis.entry(v.clone()).or_default().push(f.clone());
        }
    }
    let g = by_vis
        .values()
        .find(|v| {
            v.iter().any(|f| f.contains("/recoded/")) && v.iter().any(|f| f.contains("/original/"))
        })
        .map(|v| {
            let rec = v.iter().find(|f| f.contains("/recoded/")).unwrap().clone();
            let org = v.iter().find(|f| f.contains("/original/")).unwrap().clone();
            [org, rec]
        })
        .unwrap();
    let mut csv = String::from("fil,visuell_gruppe\n");
    // Samme scene feilmerket som to ulike motiver: det funnede paret er «falskt» mot denne fasiten.
    csv += &format!(
        "{},A\n{},B\n",
        g[0].trim_start_matches("images/"),
        g[1].trim_start_matches("images/")
    );
    let src = fjell_fs::FsSource::new(root.join("images"));
    let r = evaluate(&src, &parse_labels(&csv).unwrap(), &Options::default()).unwrap();
    assert_eq!(r.false_pair_total, 1, "{:?}", r.visual_reps);
    assert_eq!(r.false_pairs[0].truth_a, "A");

    // Fil i etikettfilen som ikke finnes: feil, ikke stille hopp.
    let bad = parse_labels("fil,uskarp\nfinnes_ikke.png,ja").unwrap();
    assert!(evaluate(&src, &bad, &Options::default()).is_err());
}
