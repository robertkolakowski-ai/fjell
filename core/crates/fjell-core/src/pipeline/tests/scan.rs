//! K7-1…4, K7-7…9 og K7-11 på syntetisk korpus (golden-korpuset dekkes i `core/testdata`).

use super::*;
use crate::cluster::{exact_groups, visual_groups, Params};
use crate::hash::{content_hash, low_info, perceptual};
use crate::model::{AlgoVer, AssetId, Hash64};
use crate::quality::blur_score;
use crate::screenshot::{signals, ShotSignals};
use crate::suggest::{
    blur_threshold, blurry_for, group_suggestions, screenshot_suggestion_default, GroupKind, Member,
};
use std::sync::atomic::Ordering::SeqCst;

/// Forventet indeks regnet med modulene hver for seg (ingen pipeline-kode).
fn expected(fx: &Fx) -> Dump {
    struct It {
        name: String,
        id: AssetId,
        content: [u8; 32],
        d: Hash64,
        p: Hash64,
        low: bool,
        blur: f32,
        member: Member,
        sig: ShotSignals,
        photo: bool,
    }
    let items: Vec<It> = fx
        .files
        .iter()
        .map(|(name, (px, bytes))| {
            let t = thumb(px.clone());
            let m = fx
                .src
                .inner
                .list_changes(None)
                .unwrap()
                .changed
                .into_iter()
                .find(|m| &m.source_id.0 == name)
                .unwrap();
            let id = fx.id(name);
            let (d, p) = perceptual(&t);
            let blur = blur_score(&t);
            It {
                name: name.clone(),
                id,
                content: content_hash(&mut &bytes[..]).unwrap(),
                d,
                p,
                low: low_info(&t),
                blur,
                member: Member {
                    id,
                    width: m.width,
                    height: m.height,
                    bytes: m.bytes,
                    blur_score: blur,
                    created_at: m.created_at,
                    protect_flags: m.protect_flags,
                },
                sig: signals(&m, &t),
                photo: m.kind == Kind::Photo,
            }
        })
        .collect();
    let by: BTreeMap<AssetId, &It> = items.iter().map(|i| (i.id, i)).collect();
    let nm = |id: AssetId| by[&id].name.clone();
    let (mut groups, mut sugg) = (BTreeSet::new(), BTreeSet::new());
    let mut reps: BTreeSet<AssetId> = items
        .iter()
        .filter(|i| i.photo && !i.low)
        .map(|i| i.id)
        .collect();
    let hashes: Vec<_> = items.iter().map(|i| (i.id, i.content)).collect();
    let mut add = |kind: &str, res: &crate::suggest::GroupResult, ids: &[AssetId]| {
        let mut ms: Vec<String> = ids.iter().map(|i| nm(*i)).collect();
        ms.sort();
        groups.insert((kind.to_string(), nm(res.keeper), ms));
        for s in &res.suggestions {
            sugg.insert((
                nm(s.asset_id),
                s.kind.as_str().to_string(),
                s.reason.key().to_string(),
                s.confidence.to_bits(),
                "pending".to_string(),
            ));
        }
    };
    for g in exact_groups(&hashes) {
        let ms: Vec<Member> = g.members.iter().map(|i| by[i].member).collect();
        let res = group_suggestions(GroupKind::Exact, &ms).unwrap();
        g.members
            .iter()
            .filter(|m| **m != res.keeper)
            .for_each(|m| {
                reps.remove(m);
            });
        add("exact", &res, &g.members);
    }
    let vi: Vec<_> = items
        .iter()
        .filter(|i| reps.contains(&i.id))
        .map(|i| (i.id, i.d, i.p))
        .collect();
    for g in visual_groups(&vi, &Params::default()) {
        let mut mx = 0;
        for a in &g.members {
            for b in &g.members {
                mx = mx
                    .max(by[a].d.hamming(by[b].d))
                    .max(by[a].p.hamming(by[b].p));
            }
        }
        let ms: Vec<Member> = g.members.iter().map(|i| by[i].member).collect();
        let res = group_suggestions(GroupKind::Visual { max_hamming: mx }, &ms).unwrap();
        add("visual", &res, &g.members);
    }
    let th = blur_threshold(&items.iter().map(|i| i.blur).collect::<Vec<_>>());
    for i in &items {
        if let (true, Some(s)) = (i.photo, blurry_for(i.id, i.blur, th)) {
            sugg.insert((
                i.name.clone(),
                "blurry".into(),
                s.reason.key().into(),
                s.confidence.to_bits(),
                "pending".into(),
            ));
        }
        if let Some(s) = screenshot_suggestion_default(i.id, i.sig) {
            sugg.insert((
                i.name.clone(),
                "screenshot".into(),
                s.reason.key().into(),
                s.confidence.to_bits(),
                "pending".into(),
            ));
        }
    }
    (groups, sugg)
}

fn corpus_fx() -> Fx {
    let mut fx = Fx::new();
    fx.corpus();
    fx
}

#[test]
fn k7_1_full_scan_equals_modules_separately() {
    let fx = corpus_fx();
    let r = fx.scan(&ScanConfig::full());
    assert!(r.failures.is_empty() && r.new == 28, "{r:?}");
    let (got, want) = (dump(&fx), expected(&fx));
    assert_eq!(got.0, want.0, "grupper");
    assert_eq!(got.1, want.1, "forslag");
    // Korpuset er laget slik at alle typer finnes (ellers sier testen ingenting).
    let kinds: BTreeSet<_> = got.1.iter().map(|s| s.1.clone()).collect();
    for k in ["exact_dup", "visual_dup", "blurry", "screenshot"] {
        assert!(kinds.contains(k), "mangler {k}: {kinds:?}");
    }
    assert_eq!(got.0.iter().filter(|g| g.0 == "exact").count(), 4);
    assert_eq!(got.0.iter().filter(|g| g.0 == "visual").count(), 3);
}

#[test]
fn k7_2_rescan_without_changes_reads_nothing() {
    let fx = corpus_fx();
    fx.scan(&ScanConfig::full());
    let before = (fx.src.counts(), dump(&fx));
    let r = fx.scan(&ScanConfig::full());
    assert_eq!(
        fx.src.counts(),
        before.0,
        "ingen miniatyrer, ingen bytelesinger"
    );
    assert_eq!(
        (r.new, r.changed, r.removed, r.hashed, r.analyzed),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(dump(&fx), before.1);
}

#[test]
fn k7_3_one_new_file_analyzes_only_it() {
    let mut fx = corpus_fx();
    fx.scan(&ScanConfig::full());
    let base = fx.src.counts();
    // Unik størrelse: ingen hashing, én miniatyr.
    fx.uniq("ny.jpg", scene(900));
    let r = fx.scan(&ScanConfig::full());
    assert_eq!((r.new, r.analyzed, r.hashed), (1, 1, 0));
    assert_eq!(fx.src.counts(), (base.0 + 1, base.1));
    // Eksakt kopi av en fil med unik størrelse: begge hashes (hash-partner), én miniatyr.
    let (px, bytes) = fx.files["u03.jpg"].clone();
    fx.put("u03-kopi.jpg", px, bytes, "t1");
    let c = fx.src.counts();
    let r = fx.scan(&ScanConfig::full());
    assert_eq!((r.new, r.analyzed, r.hashed), (1, 1, 2));
    assert_eq!(fx.src.counts(), (c.0 + 1, c.1 + 2));
    assert_eq!(dump(&fx).0.iter().filter(|g| g.0 == "exact").count(), 5);
    assert_eq!(dump(&fx), expected(&fx));
}

#[test]
fn k7_4_changed_mod_token_reanalyzes_exactly_that_file() {
    let mut fx = corpus_fx();
    fx.scan(&ScanConfig::full());
    let c = fx.src.counts();
    // Samme fil, ny token og nytt innhold (uskarp): bare den analyseres på nytt.
    let (px, bytes) = fx.files["u05.jpg"].clone();
    fx.put("u05.jpg", blurred(&px), bytes, "t2");
    let r = fx.scan(&ScanConfig::full());
    assert_eq!((r.new, r.changed, r.analyzed), (0, 1, 1));
    assert_eq!(fx.src.counts(), (c.0 + 1, c.1));
    assert_eq!(dump(&fx), expected(&fx));
}

#[test]
fn k7_7_one_bad_image_does_not_stop_the_scan() {
    let fx = corpus_fx();
    let (bad, gone, corrupt) = (
        SourceId("u01.jpg".into()),
        SourceId("u02.jpg".into()),
        SourceId("u03.jpg".into()),
    );
    fx.src.panic_on.lock().unwrap().insert(bad.clone());
    fx.src.inner.inject_failure(&gone, SourceError::Unavailable);
    fx.src.inner.inject_failure(&corrupt, SourceError::Corrupt);
    let r = fx.scan(&ScanConfig::full());
    assert_eq!(r.analyzed, 25);
    assert_eq!(r.skipped_unavailable, 1);
    let mut causes: Vec<_> = r.failures.iter().map(|f| f.cause.clone()).collect();
    causes.sort_by_key(|c| format!("{c:?}"));
    assert_eq!(causes, vec![FailCause::Corrupt, FailCause::Panic]);
    let fails = |n: &str| -> i64 {
        let id = fx.id(n).0 as i64;
        fx.with(|s| {
            s.conn()
                .query_row(
                    "SELECT COALESCE((SELECT fail_count FROM fingerprint WHERE asset_id=?1),0)",
                    [id],
                    |r| r.get(0),
                )
                .unwrap()
        })
    };
    assert_eq!(
        (fails("u01.jpg"), fails("u02.jpg"), fails("u03.jpg")),
        (1, 0, 1)
    );
    // Tre feil: hoppes over. `Unavailable` teller aldri og forsøkes hver gang.
    fx.scan(&ScanConfig::full());
    fx.scan(&ScanConfig::full());
    assert_eq!(
        (fails("u01.jpg"), fails("u02.jpg"), fails("u03.jpg")),
        (3, 0, 3)
    );
    let c = fx.src.counts().0;
    let r = fx.scan(&ScanConfig::full());
    assert_eq!(fx.src.counts().0, c + 1, "bare Unavailable-bildet forsøkes");
    assert!(r.failures.is_empty());
    // Når tilgjengelig igjen blir det analysert (uten feiltelling).
    fx.src.inner.clear_failure(&gone);
    assert_eq!(fx.scan(&ScanConfig::full()).analyzed, 1);
    assert_eq!(fails("u02.jpg"), 0);
}

#[test]
fn k7_8_exact_groups_exist_before_first_thumbnail() {
    let fx = corpus_fx();
    let seen = Arc::new(Mutex::new(Vec::<(usize, u64, Phase)>::new()));
    let prog = Arc::new(Progress::new());
    let (store, s2, p2) = (fx.store.clone(), seen.clone(), prog.clone());
    *fx.src.hook.lock().unwrap() = Some(Arc::new(Box::new(move |n| {
        if n == 0 {
            let dups = store.with_store(|s| s.exact_dup_count().unwrap());
            s2.lock().unwrap().push((n, dups, p2.snapshot().phase));
        }
    })));
    scan(
        fx.src.as_ref(),
        fx.store.as_ref(),
        &fx.clock,
        &ScanConfig::full(),
        &prog,
        &fx.cancel,
    )
    .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "første miniatyr ble hentet");
    assert_eq!(
        seen[0].1, 4,
        "fire eksakte duplikater klare før første miniatyr"
    );
    assert_eq!(seen[0].2, Phase::Analyzing);
    assert_eq!(prog.snapshot().found_exact, 4);
    // Hurtigmodus: ingen miniatyr i det hele tatt, men eksakte grupper finnes.
    let q = corpus_fx();
    let r = q.scan(&ScanConfig::quick());
    assert_eq!(q.src.counts().0, 0);
    assert_eq!(r.analyzed, 0);
    assert_eq!(dump(&q).0.iter().filter(|g| g.0 == "exact").count(), 4);
}

#[test]
fn k7_9_algo_ver_bump_analyzes_only_old_rows() {
    let fx = corpus_fx();
    assert_eq!(fx.scan_algo(1).analyzed, 28);
    assert_eq!(fx.scan_algo(1).analyzed, 0);
    // Nedgraderer tre rader: bare de analyseres.
    let ids: Vec<i64> = ["u00.jpg", "e1a.jpg", "b0.jpg"]
        .iter()
        .map(|n| fx.id(n).0 as i64)
        .collect();
    fx.with(|s| {
        for id in &ids {
            s.conn()
                .execute(
                    "UPDATE fingerprint SET algo_ver = 0 WHERE asset_id = ?1",
                    [id],
                )
                .unwrap();
        }
    });
    let c = fx.src.counts().0;
    assert_eq!(fx.scan_algo(1).analyzed, 3);
    assert_eq!(fx.src.counts().0, c + 3);
    // Økt versjon: alle gamle rader; deretter ingen.
    assert_eq!(fx.scan_algo(2).analyzed, 28);
    assert_eq!(fx.scan_algo(2).analyzed, 0);
    assert_eq!(dump(&fx), expected(&fx));
    let _ = AlgoVer(1);
}

#[test]
fn k7_11_progress_is_monotonic_and_reaches_100_percent() {
    let fx = corpus_fx();
    let snaps = Arc::new(Mutex::new(Vec::<Snapshot>::new()));
    let s2 = snaps.clone();
    let p = Progress::with_listener(
        move |s| s2.lock().unwrap().push(*s),
        std::time::Duration::ZERO,
    );
    scan(
        fx.src.as_ref(),
        fx.store.as_ref(),
        &fx.clock,
        &ScanConfig::full(),
        &p,
        &fx.cancel,
    )
    .unwrap();
    let snaps = snaps.lock().unwrap();
    assert!(snaps.len() > 5);
    let mut last = (0.0f64, Phase::Idle);
    for s in snaps.iter() {
        assert!(
            s.fraction() >= last.0 - 1e-12,
            "fremdrift gikk bakover: {s:?}"
        );
        assert!(s.phase >= last.1, "fase gikk bakover: {s:?}");
        assert!(s.fraction() <= 1.0);
        last = (s.fraction(), s.phase);
    }
    let end = snaps.last().unwrap();
    assert_eq!((end.fraction(), end.phase), (1.0, Phase::Done));
    // Totalt = filer å hashe (8) + arbeidssett (28).
    assert_eq!(end.total, 8 + 28);
}

#[test]
fn progress_listener_is_throttled_to_four_per_second() {
    let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let n2 = n.clone();
    let p = Progress::with_listener(
        move |_| {
            n2.fetch_add(1, SeqCst);
        },
        DEFAULT_INTERVAL,
    );
    let t = std::time::Instant::now();
    for _ in 0..200_000 {
        p.advance(1);
    }
    let allowed = (t.elapsed().as_millis() / 250 + 2) as usize;
    assert!(
        n.load(SeqCst) <= allowed,
        "{} oppdateringer på {:?}",
        n.load(SeqCst),
        t.elapsed()
    );
}

#[test]
fn background_priority_completes_with_same_result() {
    let a = corpus_fx();
    a.scan(&ScanConfig::full());
    let b = corpus_fx();
    b.scan(&ScanConfig {
        priority: Priority::Background,
        ..ScanConfig::full()
    });
    assert_eq!(dump(&a), dump(&b));
}

/// Formater uten dekoder (HEIC/RAW uten kodek, all video): `thumbnail` gir `Unavailable`. De skal
/// ikke telles som feil, ikke stoppe skanningen, aldri få visuelt/uskarpt/skjermbilde-forslag, men
/// fortsatt kunne være eksakte duplikater (hash av bytes). Video: bare eksakt.
#[test]
fn udekodbare_formater_og_video_gir_bare_eksakte_duplikater() {
    let fx = corpus_fx();
    let put = |name: &str, kind: Kind, mime: &str, bytes: Vec<u8>| {
        let mut m = meta(name, bytes.len() as u64, "t1");
        m.kind = kind;
        m.mime = mime.into();
        m.width = 0;
        m.height = 0;
        m.has_camera_exif = None;
        // Ingen miniatyr: `MemSource::thumbnail` svarer `Unavailable`.
        fx.src.inner.put(m, None, bytes);
    };
    put("h1.heic", Kind::Photo, "image/heic", vec![61; 7_001]);
    put("h2.heic", Kind::Photo, "image/heic", vec![61; 7_001]);
    put("raw.cr2", Kind::Photo, "image/x-canon-cr2", vec![62; 7_013]);
    put("v1.mp4", Kind::Video, "video/mp4", vec![63; 8_001]);
    put("v2.mov", Kind::Video, "video/quicktime", vec![63; 8_001]);
    // Samme størrelse som videoparet, annet innhold: ikke duplikat.
    put("v3.mp4", Kind::Video, "video/mp4", vec![64; 8_001]);
    let r = fx.scan(&ScanConfig::full());
    assert!(r.failures.is_empty(), "{:?}", r.failures);
    // Video er ikke i arbeidssettet for full fase i det hele tatt; de tre bildene hoppes over.
    assert_eq!(r.skipped_unavailable, 3);
    assert_eq!(r.analyzed, 28, "bare korpusets dekodbare bilder");
    let d = dump(&fx);
    let extra: Vec<_> =
        d.0.iter()
            .filter(|g| g.2.iter().any(|n| !n.ends_with(".jpg") && n != "shot.png"))
            .collect();
    let members: BTreeSet<Vec<String>> = extra.iter().map(|g| g.2.clone()).collect();
    let want: BTreeSet<Vec<String>> = [vec!["h1.heic", "h2.heic"], vec!["v1.mp4", "v2.mov"]]
        .into_iter()
        .map(|v| v.into_iter().map(String::from).collect())
        .collect();
    assert_eq!(members, want);
    assert!(extra.iter().all(|g| g.0 == "exact"), "{extra:?}");
    // Forslag for de nye filene: bare eksakt duplikat, ett per gruppe.
    let new_sugg: Vec<_> =
        d.1.iter()
            .filter(|s| !s.0.ends_with(".jpg") && s.0 != "shot.png")
            .collect();
    assert_eq!(new_sugg.len(), 2, "{new_sugg:?}");
    assert!(new_sugg.iter().all(|s| s.1 == "exact_dup"), "{new_sugg:?}");
    // Ny skanning prøver ikke å feile dem, og gruppene står.
    let r = fx.scan(&ScanConfig::full());
    assert!(r.failures.is_empty());
    assert_eq!(dump(&fx).0, d.0);
}
