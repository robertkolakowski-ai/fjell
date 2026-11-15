//! Golden-tester per forslagstype (V1-kalibrering): eksakt duplikat, visuelt duplikat, uskarpt, skjermbilde,
//! serie og regel, kjørt gjennom hele `Fjell` (skanning, gruppering, kort, beslutninger) mot syntetiske bilder
//! fra generatoren. Hver test viser begrunnelseskode, gruppering og keeper, og at originalene aldri
//! endres. Beskyttede bilder foreslås aldri flyttet uten bekreftelse (`AckRequired`), og alt kan angres.
//!
//! Fixtures er små og bygges i minnet (`MemSource`); ingen ekte bilder. Forutsetninger om bildene (hash-
//! avstander, skarphetsscore) sjekkes i hver test, slik at en endring i generatoren gir en tydelig feil og
//! ikke en tilfeldig grønn test.

use fjell_core::api::{
    ApiError, Card, Fjell, FjellApi, ScanConfig, ScanMode, Sensitivity, SeriesCompare,
};
use fjell_core::api::{Decision, RuleDraft};
use fjell_core::hash::perceptual;
use fjell_core::model::{
    AnalyzeError, AssetId, AssetMeta, AssetSource, ConsequenceCode, EyeAnalyzer, EyeReport,
    FakeClock, Kind, MemSource, NullEyeAnalyzer, NullFaceAnalyzer, ProtectFlags, ProtectScope,
    ReasonCode, ReasonDetail, RuleCondition, RuleSpec, SeriesId, SourceId, State, SuggestionKind,
    Thumb, ThumbFmt,
};
use fjell_core::quality::{blur_score, BLUR_ABS_MAX};
use fjell_core::screenshot::signals;
use fjell_core::store::Store;
use fjell_testdata::calib::{gray_thumb, Gray};
use fjell_testdata::ops::{encode_jpeg, encode_png, gaussian, resize_area, Img};
use fjell_testdata::rng::fnv64;
use fjell_testdata::synth::{lowcontrast, scene, ui_screen};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

const T0: i64 = 1_700_000_000;

fn thumb_of(g: &Gray) -> Thumb {
    Thumb::new(g.w as u32, g.h as u32, ThumbFmt::Gray8, g.px.clone()).unwrap()
}

/// Ett bilde som skal inn i kilden.
struct Photo {
    name: &'static str,
    bytes: Vec<u8>,
    thumb: Gray,
    width: u32,
    height: u32,
    mime: &'static str,
    created_at: i64,
    hint: bool,
    camera: Option<bool>,
}

impl Photo {
    fn png(name: &'static str, img: &Img, created_at: i64) -> Photo {
        Photo::of(name, img, encode_png(img), "image/png", created_at)
    }
    fn jpeg(name: &'static str, img: &Img, created_at: i64) -> Photo {
        Photo::of(name, img, encode_jpeg(img, 90), "image/jpeg", created_at)
    }
    fn of(name: &'static str, img: &Img, bytes: Vec<u8>, mime: &'static str, t: i64) -> Photo {
        Photo {
            name,
            bytes,
            thumb: gray_thumb(img),
            width: img.w as u32,
            height: img.h as u32,
            mime,
            created_at: t,
            hint: false,
            camera: Some(true),
        }
    }
    fn screenshot_meta(mut self, hint: bool, camera: Option<bool>) -> Photo {
        self.hint = hint;
        self.camera = camera;
        self
    }
}

/// Analysator som svarer med faste rapporter per miniatyr (nøkkel: FNV av pikslene).
struct FixedEyes(BTreeMap<u64, EyeReport>);

impl EyeAnalyzer for FixedEyes {
    fn analyze(&self, t: &Thumb) -> Result<EyeReport, AnalyzeError> {
        Ok(self.0.get(&fnv64(&t.data)).copied().unwrap_or_default())
    }
}

struct World {
    f: Fjell,
    src: Arc<MemSource>,
    names: Vec<&'static str>,
}

fn put_photo(src: &MemSource, p: Photo) {
    let meta = AssetMeta {
        source_id: SourceId(format!("/Pictures/{}", p.name)),
        kind: Kind::Photo,
        created_at: p.created_at,
        mod_token: "1".into(),
        width: p.width,
        height: p.height,
        bytes: p.bytes.len() as u64,
        mime: p.mime.into(),
        platform_screenshot_hint: p.hint,
        has_camera_exif: p.camera,
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    };
    src.put(meta, Some(thumb_of(&p.thumb)), p.bytes);
}

fn world_with(photos: Vec<Photo>, eye: Arc<dyn EyeAnalyzer>) -> World {
    let src = Arc::new(MemSource::new());
    let mut names = Vec::new();
    for p in photos {
        names.push(p.name);
        put_photo(&src, p);
    }
    let f = Fjell::with_analyzers(
        Store::open_in_memory().unwrap(),
        src.clone(),
        Arc::new(FakeClock::new(T0 + 10_000_000)),
        eye,
        Arc::new(NullFaceAnalyzer),
    );
    scan(&f);
    World { f, src, names }
}

fn world(photos: Vec<Photo>) -> World {
    world_with(photos, Arc::new(NullEyeAnalyzer))
}

fn scan(f: &Fjell) {
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.failures.is_empty(), "{r:?}");
}

impl World {
    /// Flere bilder kommer til etter første skanning; ny skanning.
    fn add(&mut self, photos: Vec<Photo>) {
        for p in photos {
            self.names.push(p.name);
            put_photo(&self.src, p);
        }
        scan(&self.f);
    }

    /// Navn -> id (id-ene er 1..=n i den rekkefølgen bildene ble indeksert).
    fn ids(&self) -> BTreeMap<&'static str, AssetId> {
        let ids: Vec<AssetId> = (1..=self.names.len() as u64).map(AssetId).collect();
        let src = self.f.sources_for(&ids);
        src.into_iter()
            .map(|(id, s)| {
                let n = self.names.iter().find(|n| s.0 == format!("/Pictures/{n}"));
                (*n.expect("kjent kilde"), id)
            })
            .collect()
    }

    fn cards(&self, kind: SuggestionKind) -> Vec<Card> {
        let mut out = Vec::new();
        let mut cur = None;
        loop {
            let p = self.f.cards(kind, 7, cur).unwrap();
            out.extend(p.cards);
            match p.next {
                Some(n) => cur = Some(n),
                None => return out,
            }
        }
    }

    fn all_cards(&self) -> Vec<Card> {
        SuggestionKind::ALL
            .iter()
            .flat_map(|k| self.cards(*k))
            .collect()
    }

    fn state(&self, id: AssetId) -> State {
        self.f
            .with_store(|s| s.asset_row(id).unwrap().unwrap().state)
    }

    /// Fingeravtrykk av alle originalfiler i kilden (innhold, ikke metadata).
    fn content(&self) -> BTreeMap<String, u64> {
        let cs = self.src.list_changes(None).unwrap();
        cs.changed
            .iter()
            .map(|m| {
                let mut r = self.src.read_bytes(&m.source_id).unwrap();
                let mut b = Vec::new();
                r.read_to_end(&mut b).unwrap();
                (m.source_id.0.clone(), fnv64(&b))
            })
            .collect()
    }

    fn keeper_reason(&self, kind: SuggestionKind) -> Vec<Option<String>> {
        self.f.with_store(|s| {
            s.pending_cards(kind, 100, None)
                .unwrap()
                .into_iter()
                .map(|r| r.keeper_reason)
                .collect()
        })
    }
}

fn sharp(img: &Img) -> Img {
    img.clone()
}

/// Hver kortide skal bare finnes i ett kort (kortprioritet), og ingen keeper har kort.
fn assert_no_double_cards(w: &World) {
    let mut seen = std::collections::BTreeSet::new();
    for c in w.all_cards() {
        assert!(seen.insert(c.asset), "asset i flere kort: {c:?}");
        assert_ne!(Some(c.asset), c.keeper, "keeper har kort: {c:?}");
    }
}

// --- eksakt duplikat ---------------------------------------------------------------------------------

#[test]
fn exact_duplicate_reason_group_keeper_and_originals_untouched() {
    let img = scene(11, 512, 384);
    let other = scene(12, 512, 384);
    let w = world(vec![
        Photo::png("a_original.png", &img, T0),
        Photo::png("b_kopi.png", &img, T0 + 3600),
        Photo::png("c_kopi.png", &img, T0 + 7200),
        Photo::png("d_annet.png", &other, T0 + 100),
    ]);
    let before = w.content();
    let id = w.ids();
    let cards = w.cards(SuggestionKind::ExactDup);
    assert_eq!(cards.len(), 2, "{cards:?}");
    for c in &cards {
        assert_eq!(c.reason, ReasonCode::ExactCopy);
        assert_eq!(c.confidence, 1.0);
        assert_eq!(c.keeper, Some(id["a_original.png"]), "eldst beholdes");
        assert_eq!(c.group, cards[0].group, "samme gruppe");
    }
    let marked: Vec<AssetId> = cards.iter().map(|c| c.asset).collect();
    assert!(marked.contains(&id["b_kopi.png"]) && marked.contains(&id["c_kopi.png"]));
    assert_eq!(
        w.keeper_reason(SuggestionKind::ExactDup)[0].as_deref(),
        Some(ReasonCode::Older.key())
    );
    assert!(w.cards(SuggestionKind::VisualDup).is_empty());
    assert_no_double_cards(&w);
    assert_eq!(w.content(), before, "originalene er uendret");
    assert!(w.names.iter().all(|n| w.state(id[n]) == State::Active));
}

// --- visuelt duplikat --------------------------------------------------------------------------------

#[test]
fn visual_duplicate_keeps_largest_and_gives_visual_reason() {
    let img = scene(21, 512, 384);
    let small = resize_area(&img, 256, 192);
    let unrelated = scene(22, 512, 384);
    let w = world(vec![
        Photo::png("a_stor.png", &img, T0 + 3600),
        Photo::jpeg("b_liten.jpg", &small, T0),
        Photo::png("c_annet.png", &unrelated, T0 + 50),
    ]);
    let id = w.ids();
    let (da, pa) = perceptual(&thumb_of(&gray_thumb(&img)));
    let (db, pb) = perceptual(&thumb_of(&gray_thumb(&small)));
    assert!(
        da.hamming(db) <= 6 && pa.hamming(pb) <= 6,
        "forutsetning: nedskalert kopi er innen Normal"
    );
    let cards = w.cards(SuggestionKind::VisualDup);
    assert_eq!(cards.len(), 1, "{cards:?}");
    let c = &cards[0];
    assert_eq!(c.asset, id["b_liten.jpg"]);
    assert_eq!(c.reason, ReasonCode::VisualDuplicate);
    assert_eq!(
        c.keeper,
        Some(id["a_stor.png"]),
        "største beholdes selv om den er yngst"
    );
    assert!(c.confidence > 0.0 && c.confidence <= 1.0);
    assert_eq!(
        w.keeper_reason(SuggestionKind::VisualDup)[0].as_deref(),
        Some(ReasonCode::HigherResolution.key())
    );
    assert!(w.cards(SuggestionKind::ExactDup).is_empty());
    assert!(w.cards(SuggestionKind::Series).is_empty());
    assert_no_double_cards(&w);
}

// --- uskarpt -----------------------------------------------------------------------------------------

#[test]
fn blurry_flagged_sharp_and_low_contrast_not() {
    let sharp_img = sharp(&scene(31, 512, 384));
    let blurred = gaussian(&scene(32, 512, 384), 8.0);
    let flat_but_sharp = lowcontrast(&scene(33, 512, 384), 0.35);
    let score = |i: &Img| blur_score(&thumb_of(&gray_thumb(i)));
    assert!(score(&sharp_img) > BLUR_ABS_MAX, "forutsetning: skarpt");
    assert!(score(&blurred) < BLUR_ABS_MAX, "forutsetning: uskarpt");
    assert!(
        score(&flat_but_sharp) > BLUR_ABS_MAX,
        "forutsetning: lav kontrast er ikke uskarphet"
    );
    let w = world(vec![
        Photo::jpeg("a_skarp.jpg", &sharp_img, T0),
        Photo::jpeg("b_uskarp.jpg", &blurred, T0 + 7200),
        Photo::jpeg("c_lavkontrast.jpg", &flat_but_sharp, T0 + 14_400),
    ]);
    let id = w.ids();
    let cards = w.cards(SuggestionKind::Blurry);
    assert_eq!(cards.len(), 1, "{cards:?}");
    assert_eq!(cards[0].asset, id["b_uskarp.jpg"]);
    assert_eq!(cards[0].reason, ReasonCode::Blurry);
    assert_eq!(cards[0].group, None, "uskarpt har ingen gruppe");
    assert!(cards[0].confidence >= 0.5 && cards[0].confidence <= 1.0);
    assert_eq!(w.all_cards().len(), 1);
}

// --- skjermbilde -------------------------------------------------------------------------------------

#[test]
fn screenshot_reason_signals_and_negatives() {
    let shot = ui_screen(41, 1170, 2532);
    let photo = scene(42, 1024, 768);
    let edited = scene(43, 900, 700); // redigert foto, png uten exif: bare to svake signaler
    let w = world(vec![
        Photo::png("a_skjerm.png", &shot, T0).screenshot_meta(true, Some(false)),
        Photo::jpeg("b_kamera.jpg", &photo, T0 + 3600).screenshot_meta(false, Some(true)),
        Photo::png("c_redigert.png", &edited, T0 + 7200).screenshot_meta(false, Some(false)),
    ]);
    let id = w.ids();
    let cards = w.cards(SuggestionKind::Screenshot);
    assert_eq!(cards.len(), 1, "{cards:?}");
    assert_eq!(cards[0].asset, id["a_skjerm.png"]);
    assert_eq!(cards[0].reason, ReasonCode::Screenshot);
    assert!((cards[0].confidence - 0.99).abs() < 1e-6, "fire signaler");
    // Begrunnelsesmerkene bak kortet: alle fire signaler for skjermbildet, to for det redigerte.
    let meta = |hint, w, h, mime: &str, camera| AssetMeta {
        source_id: SourceId("x".into()),
        kind: Kind::Photo,
        created_at: 0,
        mod_token: "1".into(),
        width: w,
        height: h,
        bytes: 1,
        mime: mime.into(),
        platform_screenshot_hint: hint,
        has_camera_exif: camera,
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    };
    let dummy = Thumb::new(8, 8, ThumbFmt::Gray8, vec![0; 64]).unwrap();
    let s = signals(&meta(true, 1170, 2532, "image/png", Some(false)), &dummy);
    assert_eq!(
        s.reasons(),
        vec![
            ReasonCode::ShotPlatformFlag,
            ReasonCode::ShotScreenSize,
            ReasonCode::ShotNoCameraExif,
            ReasonCode::ShotPng
        ]
    );
    let e = signals(&meta(false, 900, 700, "image/png", Some(false)), &dummy);
    assert_eq!(e.count(), 2);
    assert_eq!(e.suggest_default(), None, "uten S1/S2 foreslås ikke");
    assert_eq!(
        e.suggestion(false),
        Some(0.70),
        "løs regel ville tatt det med"
    );
    assert_no_double_cards(&w);
}

// --- serie -------------------------------------------------------------------------------------------

/// Lysere eller mørkere rektangler oppå motivet: «noe flyttet seg» mellom bildene i en serie.
fn marked(base: &Img, rects: &[(usize, usize, usize, usize, i32)]) -> Img {
    let mut o = base.clone();
    for &(x0, y0, rw, rh, off) in rects {
        for y in y0..y0 + rh {
            for x in x0..x0 + rw {
                let i = (y * base.w + x) * 3;
                for c in 0..3 {
                    o.rgb[i + c] = (o.rgb[i + c] as i32 + off).clamp(0, 255) as u8;
                }
            }
        }
    }
    o
}

/// Tre serie-bilder (innbyrdes avstand 8..=10) pluss to ekstra: `later` (10 fra to av dem, brukes lenge etter) og
/// `late15` (8 fra det midterste, brukes 15 s etter serien).
struct SeriesImgs {
    s: [Img; 3],
    later: Img,
    late15: Img,
}

fn series_images() -> SeriesImgs {
    let base = scene(55, 512, 384);
    SeriesImgs {
        s: [
            marked(&base, &[(60, 60, 80, 70, 80)]),
            marked(&base, &[(300, 200, 80, 70, -80)]),
            marked(&base, &[(120, 250, 70, 70, 70)]),
        ],
        later: marked(&base, &[(200, 20, 80, 70, 80)]),
        late15: marked(&base, &[(250, 120, 80, 70, 80)]),
    }
}

fn dist(a: &Img, b: &Img) -> u32 {
    let (da, pa) = perceptual(&thumb_of(&gray_thumb(a)));
    let (db, pb) = perceptual(&thumb_of(&gray_thumb(b)));
    da.hamming(db).max(pa.hamming(pb))
}

fn series_photos(imgs: &SeriesImgs) -> Vec<Photo> {
    vec![
        Photo::jpeg("s1.jpg", &imgs.s[0], T0),
        Photo::jpeg("s2.jpg", &imgs.s[1], T0 + 3),
        Photo::jpeg("s3.jpg", &imgs.s[2], T0 + 7),
        // Samme motiv en time senere: aldri med i denne serien.
        Photo::jpeg("s4_senere.jpg", &imgs.later, T0 + 7 + 3600),
    ]
}

fn series_of(w: &World) -> (Vec<Card>, SeriesCompare) {
    let cards = w.cards(SuggestionKind::Series);
    assert!(!cards.is_empty(), "ingen serie");
    let sid = SeriesId(cards[0].group.unwrap().0);
    assert!(cards.iter().all(|c| c.group.unwrap().0 == sid.0));
    let cmp = w.f.series_compare(sid).unwrap();
    (cards, cmp)
}

#[test]
fn series_groups_keeps_sharpest_and_excludes_far_in_time() {
    let imgs = series_images();
    for (a, b) in [(0, 1), (1, 2), (0, 2)] {
        let d = dist(&imgs.s[a], &imgs.s[b]);
        assert!(
            (7..=10).contains(&d),
            "forutsetning: bildene er serie, ikke duplikat ({a},{b}) = {d}"
        );
    }
    let w = world(series_photos(&imgs));
    let id = w.ids();
    let (cards, cmp) = series_of(&w);
    assert_eq!(cmp.members.len(), 3, "tre bilder i serien");
    assert!(cmp.members.iter().all(|m| m.asset != id["s4_senere.jpg"]));
    assert!(cmp.members[0].is_keeper && cmp.members[0].asset == cmp.keeper);
    assert_eq!(cmp.members[0].reason, ReasonCode::KeeperBestOverall);
    assert!(cmp.members.windows(2).all(|p| p[0].score >= p[1].score));
    // Keeper er det skarpeste bildet.
    let scores: Vec<(f32, AssetId)> = imgs
        .s
        .iter()
        .zip(["s1.jpg", "s2.jpg", "s3.jpg"])
        .map(|(i, n)| (blur_score(&thumb_of(&gray_thumb(i))), id[n]))
        .collect();
    let best = scores
        .iter()
        .copied()
        .fold((f32::MIN, AssetId(0)), |b, x| if x.0 > b.0 { x } else { b });
    assert_eq!(cmp.keeper, best.1);
    // Kortene: de to andre, med posisjon som begrunnelse, og keeper har ikke kort.
    assert_eq!(cards.len(), 2);
    for c in &cards {
        assert_ne!(c.asset, cmp.keeper);
        assert_eq!(c.keeper, Some(cmp.keeper));
        assert_eq!(c.reason, ReasonCode::SeriesMember);
        assert!(
            matches!(c.detail, ReasonDetail::Position { of: 3, .. }),
            "{c:?}"
        );
    }
    assert!(w.cards(SuggestionKind::VisualDup).is_empty());
    assert_no_double_cards(&w);
}

/// Øyne i serier. Merk: serien lagres med keeper første gang den dannes og beholder keeperen så lenge medlemmene
/// er de samme (hensyn til brukerens valg). Øyeanalysen kjøres etter skanningen, så for å få øyne med i selve
/// valget kommer bildene her i to omganger: s3 (analysert) først, så s1, s2 og det sene bildet.
fn eyes_world(conf: f32) -> World {
    let imgs = series_images();
    let key = |i: &Img| fnv64(&gray_thumb(i).px);
    let mut m = BTreeMap::new();
    let open = EyeReport {
        faces: 1,
        closed: 0,
        confidence: 0.95,
    };
    m.insert(key(&imgs.s[0]), open);
    m.insert(key(&imgs.s[1]), open);
    m.insert(
        key(&imgs.s[2]),
        EyeReport {
            faces: 1,
            closed: 1,
            confidence: conf,
        },
    );
    let mut photos = series_photos(&imgs);
    let first = vec![photos.remove(2)]; // s3
    let mut w = world_with(first, Arc::new(FixedEyes(m)));
    w.add(photos);
    w
}

#[test]
fn series_closed_eyes_move_keeper_only_above_confidence() {
    // Sikker «lukkede øyne» (0,95 >= 0,80) på det skarpeste bildet (s3): keeper byttes til s1 med
    // begrunnelse «øyne åpne», og s3 forklares med «lukkede øyne» og rangeres sist.
    let w = eyes_world(0.95);
    let id = w.ids();
    let (cards, cmp) = series_of(&w);
    assert_eq!(cmp.keeper, id["s1.jpg"], "{cmp:?}");
    assert_eq!(cmp.members[0].reason, ReasonCode::KeeperEyesOpen);
    let last = cmp.members.last().unwrap();
    assert_eq!(last.asset, id["s3.jpg"], "lukkede øyne rangeres sist");
    assert_eq!(last.reason, ReasonCode::ClosedEyes);
    assert_eq!(last.detail, ReasonDetail::Count(1));
    let closed_card = cards.iter().find(|c| c.asset == id["s3.jpg"]).unwrap();
    assert_eq!(closed_card.reason, ReasonCode::ClosedEyes);
    assert_eq!(closed_card.detail, ReasonDetail::Count(1));
    assert_no_double_cards(&w);

    // Usikker (0,50 < 0,80): øynene regnes som ukjente, aldri som lukkede; keeper er det skarpeste (s3).
    let w = eyes_world(0.50);
    let id = w.ids();
    let (_, cmp) = series_of(&w);
    assert_eq!(cmp.keeper, id["s3.jpg"]);
    assert_eq!(cmp.members[0].reason, ReasonCode::KeeperBestOverall);
    assert!(cmp
        .members
        .iter()
        .all(|m| m.reason != ReasonCode::ClosedEyes));
}

#[test]
fn series_gap_follows_sensitivity() {
    let imgs = series_images();
    // Siste bilde er 15 s etter det forrige: utenfor Normal (10 s), innenfor High (20 s), utenfor Low (5 s).
    let photos = {
        let mut p = series_photos(&imgs);
        p.pop();
        p.push(Photo::jpeg("s4_15s.jpg", &imgs.late15, T0 + 7 + 15));
        p
    };
    let w = world(photos);
    let id = w.ids();
    let members = |w: &World| -> usize { series_of(w).1.members.len() };
    assert_eq!(members(&w), 3, "Normal: 15 s er for langt");
    let mut s = w.f.settings();
    s.sensitivity.set(SuggestionKind::Series, Sensitivity::High);
    w.f.set_settings(s);
    scan(&w.f);
    w.f.sync_groups().unwrap();
    let (_, cmp) = series_of(&w);
    assert!(
        cmp.members.iter().any(|m| m.asset == id["s4_15s.jpg"]),
        "High: 15 s er innenfor 20 s: {cmp:?}"
    );
    s.sensitivity.set(SuggestionKind::Series, Sensitivity::Low);
    w.f.set_settings(s);
    scan(&w.f);
    w.f.sync_groups().unwrap();
    // Low (5 s og Hamming 8): 15 s-bildet er langt utenfor. Serien kan ha falt helt fra hverandre
    // (bildeparene er 9 og 10 fra hverandre), og da finnes det ingen serie å vise.
    let cards = w.cards(SuggestionKind::Series);
    assert!(
        cards.iter().all(|c| c.asset != id["s4_15s.jpg"]),
        "{cards:?}"
    );
}

// --- regel -------------------------------------------------------------------------------------------

#[test]
fn rule_suggests_with_rule_reason_and_never_moves_anything() {
    let w = world(vec![
        Photo::png("a.png", &scene(61, 512, 384), T0),
        Photo::jpeg("b.jpg", &scene(62, 512, 384), T0 + 3600),
        Photo::png("c.png", &scene(63, 384, 512), T0 + 7200),
    ]);
    let before = w.content();
    let id = w.ids();
    let spec = RuleSpec {
        all_of: vec![RuleCondition::Mime("image/png".into())],
    };
    assert_eq!(w.f.rule_preview(spec.clone()).unwrap().hits, 2);
    assert!(
        w.cards(SuggestionKind::Rule).is_empty(),
        "forhåndsvisning lagrer ikke"
    );
    let rid =
        w.f.rule_save(RuleDraft {
            id: None,
            name: "PNG".into(),
            spec,
        })
        .unwrap();
    let cards = w.cards(SuggestionKind::Rule);
    assert_eq!(cards.len(), 2, "{cards:?}");
    for c in &cards {
        assert_eq!(c.reason, ReasonCode::Rule);
        assert_eq!(c.detail, ReasonDetail::Rule(rid));
        assert_eq!(c.group, None);
    }
    let hit: Vec<AssetId> = cards.iter().map(|c| c.asset).collect();
    assert!(hit.contains(&id["a.png"]) && hit.contains(&id["c.png"]));
    // Regelen foreslår bare: ingenting er flyttet, innholdet er uendret, og et slått av regel gir ingen kort.
    assert!(w.names.iter().all(|n| w.state(id[n]) == State::Active));
    assert_eq!(w.content(), before);
    w.f.rule_toggle(rid, false).unwrap();
    assert!(w.cards(SuggestionKind::Rule).is_empty());
}

// --- beskyttede bilder -------------------------------------------------------------------------------

#[test]
fn protected_never_moved_without_ack_and_everything_can_be_undone() {
    let img = scene(71, 512, 384);
    let w = world(vec![
        Photo::png("a_original.png", &img, T0),
        Photo::png("b_kopi.png", &img, T0 + 3600),
        Photo::png("c_kopi.png", &img, T0 + 7200),
        Photo::png("skjerm/d_skjerm.png", &ui_screen(72, 1170, 2532), T0 + 9000)
            .screenshot_meta(true, Some(false)),
    ]);
    let before = w.content();
    let id = w.ids();
    let dup = w
        .cards(SuggestionKind::ExactDup)
        .into_iter()
        .find(|c| c.asset == id["b_kopi.png"])
        .unwrap();
    // Beskytter kopien: å flytte den krever eksplisitt bekreftelse; uten den skjer ingenting.
    w.f.protect_set(ProtectScope::Asset(id["b_kopi.png"]))
        .unwrap();
    let r = w.f.decide_many(vec![(dup.id, Decision::Trash)]);
    let Err(ApiError::AckRequired(codes)) = r else {
        panic!("forventet AckRequired: {r:?}");
    };
    assert!(
        codes.contains(&ConsequenceCode::ProtectedByScope),
        "{codes:?}"
    );
    assert_eq!(w.state(id["b_kopi.png"]), State::Active);
    // Samme for skjermbilde i beskyttet mappe: forslag uten gruppe krever også bekreftelse.
    let shot = w.cards(SuggestionKind::Screenshot).pop().unwrap();
    w.f.protect_set(ProtectScope::Folder("/Pictures/skjerm".into()))
        .unwrap();
    let r = w.f.decide_many(vec![(shot.id, Decision::Trash)]);
    assert!(
        matches!(&r, Err(ApiError::AckRequired(c)) if c.contains(&ConsequenceCode::ProtectedByScope)),
        "{r:?}"
    );
    assert_eq!(w.state(id["skjerm/d_skjerm.png"]), State::Active);
    // Regler treffer aldri beskyttede bilder.
    let spec = RuleSpec {
        all_of: vec![RuleCondition::Mime("image/png".into())],
    };
    w.f.rule_save(RuleDraft {
        id: None,
        name: "PNG".into(),
        spec,
    })
    .unwrap();
    let rule_hits: Vec<AssetId> = w
        .cards(SuggestionKind::Rule)
        .iter()
        .map(|c| c.asset)
        .collect();
    assert!(!rule_hits.contains(&id["b_kopi.png"]));
    assert!(!rule_hits.contains(&id["skjerm/d_skjerm.png"]));
    // Med bekreftelse går det: bildet blir «nylig slettet» (ikke hard sletting), og alt kan angres.
    let ok = w.f.decide_many_confirmed(
        vec![(dup.id, Decision::Trash)],
        vec![ConsequenceCode::ProtectedByScope],
    );
    if let Err(ApiError::AckRequired(c)) = &ok {
        // Flere konsekvenser kan kreve svar (f.eks. sikkerhetskopi ukjent): bekreft alle som ble nevnt.
        let all = c.clone();
        w.f.decide_many_confirmed(vec![(dup.id, Decision::Trash)], all)
            .unwrap();
    } else {
        ok.unwrap();
    }
    assert_eq!(w.state(id["b_kopi.png"]), State::Trashed);
    assert_eq!(
        w.content(),
        before,
        "innholdet i originalene er uendret også etter flytting"
    );
    w.f.undo().unwrap();
    assert_eq!(w.state(id["b_kopi.png"]), State::Active);
    assert!(w.names.iter().all(|n| w.state(id[n]) == State::Active));
}
