//! Golden-port K7-1: full skanning av korpuset gir samme grupper og forslag som modulene hver for seg.
//! Pipeline kjøres via `Fjell` (offentlig api) mot en `MemSource` fylt fra korpuset; fasiten regnes med
//! `exact_groups`, `visual_groups`, `group_suggestions`, `blur_threshold` m.fl. uten pipeline-kode.
//! Kort er deduplisert mellom typer (exact_dup > visual_dup > screenshot > blurry), også i fasiten.

use fjell_core::api::{Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::cluster::{exact_groups, visual_groups, Params};
use fjell_core::hash::{content_hash, low_info, perceptual};
use fjell_core::model::{
    AssetId, AssetMeta, FakeClock, Hash64, Kind, MemSource, ProtectFlags, SourceId, SuggestionKind,
    Thumb, ThumbFmt,
};
use fjell_core::quality::blur_score;
use fjell_core::screenshot::signals;
use fjell_core::store::Store;
use fjell_core::suggest::{
    blur_threshold, blurry_suggestion, group_suggestions, screenshot_suggestion_default, GroupKind,
    Member,
};
use fjell_testdata::corpus::{generate, DEFAULT_SEED};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::{fs, path::PathBuf};

struct It {
    id: AssetId,
    name: String,
    content: [u8; 32],
    d: Hash64,
    p: Hash64,
    low: bool,
    blur: f32,
    member: Member,
    sig: fjell_core::screenshot::ShotSignals,
}

#[test]
fn k7_1_pipeline_equals_modules_on_golden_corpus() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden_pipeline");
    let _ = fs::remove_dir_all(&dir);
    let m = generate(&dir, DEFAULT_SEED).unwrap();
    let src = Arc::new(MemSource::new());
    let mut items = Vec::new();
    for (i, e) in m.entries.iter().enumerate() {
        let bytes = fs::read(dir.join(&e.file)).unwrap();
        let img = image::open(dir.join(&e.thumb)).unwrap().to_luma8();
        let t = Thumb::new(img.width(), img.height(), ThumbFmt::Gray8, img.into_raw()).unwrap();
        let meta = AssetMeta {
            source_id: SourceId(e.file.clone()),
            kind: Kind::Photo,
            created_at: e.created_at,
            mod_token: "1".into(),
            width: e.width,
            height: e.height,
            bytes: bytes.len() as u64,
            mime: e.mime.clone(),
            platform_screenshot_hint: e.platform_screenshot_hint,
            has_camera_exif: e.has_camera_exif,
            is_local: true,
            protect_flags: ProtectFlags::default(),
            albums: vec![],
            backup: None,
        };
        let id = AssetId(i as u64 + 1); // MemSource listes sortert på source_id, så id settes under
        let (d, p) = perceptual(&t);
        let blur = blur_score(&t);
        items.push((
            meta.clone(),
            It {
                id,
                name: e.file.clone(),
                content: content_hash(&mut &bytes[..]).unwrap(),
                d,
                p,
                low: low_info(&t),
                blur,
                member: Member {
                    id,
                    width: meta.width,
                    height: meta.height,
                    bytes: meta.bytes,
                    blur_score: blur,
                    created_at: meta.created_at,
                    protect_flags: meta.protect_flags,
                },
                sig: signals(&meta, &t),
            },
        ));
        src.put(meta, Some(t), bytes);
    }
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        src,
        Arc::new(FakeClock::new(1)),
    );
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.failures.is_empty(), "{:?}", r.failures);

    // Pipeline-id-er følger listerekkefølgen (`MemSource` sorterer på source_id), startende på 1.
    let mut names: Vec<String> = items.iter().map(|(_, it)| it.name.clone()).collect();
    names.sort();
    let db_id: BTreeMap<String, AssetId> = names
        .into_iter()
        .enumerate()
        .map(|(i, n)| (n, AssetId(i as u64 + 1)))
        .collect();
    let mut its: Vec<It> = items
        .into_iter()
        .map(|(_, mut it)| {
            let id = db_id[&it.name];
            it.id = id;
            it.member.id = id;
            it
        })
        .collect();
    its.sort_by_key(|i| i.id);
    let by: BTreeMap<AssetId, &It> = its.iter().map(|i| (i.id, i)).collect();

    // Fasit fra modulene.
    let (mut exact, mut visual) = (BTreeSet::new(), BTreeSet::new());
    let (mut ex_dup, mut vis_dup) = (BTreeSet::new(), BTreeSet::new());
    let mut reps: BTreeSet<AssetId> = its.iter().filter(|i| !i.low).map(|i| i.id).collect();
    let hs: Vec<_> = its.iter().map(|i| (i.id, i.content)).collect();
    for g in exact_groups(&hs) {
        let ms: Vec<Member> = g.members.iter().map(|i| by[i].member).collect();
        let res = group_suggestions(GroupKind::Exact, &ms).unwrap();
        for s in &res.suggestions {
            ex_dup.insert(s.asset_id);
            reps.remove(&s.asset_id);
        }
        exact.insert(g.members.clone());
    }
    let vi: Vec<_> = its
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
        vis_dup.extend(res.suggestions.iter().map(|s| s.asset_id));
        visual.insert(g.members.clone());
    }
    let th = blur_threshold(&its.iter().map(|i| i.blur).collect::<Vec<_>>());
    let blurry: BTreeSet<AssetId> = its
        .iter()
        .filter(|i| blurry_suggestion(i.blur, th).is_some())
        .map(|i| i.id)
        .collect();
    let shots: BTreeSet<AssetId> = its
        .iter()
        .filter(|i| screenshot_suggestion_default(i.id, i.sig).is_some())
        .map(|i| i.id)
        .collect();
    // Dedupliser som `cards`: exact > visual > screenshot > blurry.
    let mut taken = ex_dup.clone();
    let vis_dup: BTreeSet<_> = vis_dup.difference(&taken).copied().collect();
    taken.extend(&vis_dup);
    let shots: BTreeSet<_> = shots.difference(&taken).copied().collect();
    taken.extend(&shots);
    let blurry: BTreeSet<_> = blurry.difference(&taken).copied().collect();

    // Fra api.
    let mut got: HashMap<SuggestionKind, BTreeSet<AssetId>> = HashMap::new();
    let mut groups: HashMap<SuggestionKind, BTreeMap<u64, BTreeSet<AssetId>>> = HashMap::new();
    for k in SuggestionKind::ALL {
        let mut cur = None;
        loop {
            let p = f.cards(k, 50, cur).unwrap();
            for c in &p.cards {
                got.entry(k).or_default().insert(c.asset);
                if let (Some(g), Some(kp)) = (c.group, c.keeper) {
                    let e = groups.entry(k).or_default().entry(g.0).or_default();
                    e.insert(c.asset);
                    e.insert(kp);
                }
            }
            match p.next {
                Some(n) => cur = Some(n),
                None => break,
            }
        }
    }
    let g = |k| got.get(&k).cloned().unwrap_or_default();
    assert_eq!(g(SuggestionKind::ExactDup), ex_dup);
    assert_eq!(g(SuggestionKind::VisualDup), vis_dup);
    assert_eq!(g(SuggestionKind::Screenshot), shots);
    assert_eq!(g(SuggestionKind::Blurry), blurry);
    let as_groups = |k| -> BTreeSet<Vec<AssetId>> {
        groups
            .get(&k)
            .map(|m| m.values().map(|s| s.iter().copied().collect()).collect())
            .unwrap_or_default()
    };
    assert_eq!(as_groups(SuggestionKind::ExactDup), exact);
    // Visuelle grupper vises bare for ikke-deduplisert medlem; hver vist gruppe må være en fasitgruppe.
    for grp in as_groups(SuggestionKind::VisualDup) {
        assert!(
            visual.iter().any(|v| grp.iter().all(|a| v.contains(a))),
            "ukjent visuell gruppe {grp:?}"
        );
    }
    eprintln!(
        "K7-1: {} eksakte grupper, {} visuelle grupper, {} uskarpe, {} skjermbilder, {} bilder",
        exact.len(),
        visual.len(),
        blurry.len(),
        shots.len(),
        its.len()
    );
    assert!(!exact.is_empty() && !visual.is_empty() && !blurry.is_empty() && !shots.is_empty());
}
