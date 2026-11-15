//! Konformitet: scenariene i `conformance_v1.json` kjøres mot ekte kjerne (`Fjell`). Samme fil kjøres mot
//! `MockBridge` av `desktop/src/bridge/conformance.test.ts`; begge må gi lik utgang for beskyttede bilder
//! (`ack_required`, `SCOPE_FLAG`), serie- og regelkandidater, wipe av ansiktsindeksen og feilkodene.
//! Kjernen er sannheten: avviker mocken, rettes mocken.
//!
//! Feilkoder normaliseres slik UI skal se dem: `ApiError::Internal(<V1-kode>)` blir koden selv (skallet sender
//! i dag `core_error` med koden som detalj; se UX-funnet i docs/v1/V1-PLAN.md), `AckRequired` blir `ack_required`
//! med konsekvenskodene kommaseparert i detaljen (som `ErrorDto`).
//!
//! Grensesnitt: bare tester. Ingen serde i kjernen, så filen leses med en liten JSON-leser her.

use super::super::fjell_v1::SCOPE_FLAG;
use super::scope::source_id_of;
use super::*;
use crate::model::{CardId, PersonId, RuleId};
use std::collections::BTreeSet;

const FIXTURE: &str = include_str!("conformance_v1.json");
const MISSING: u64 = 4_000_000_000;

// ---- minimal JSON-leser (bare det fixturen bruker) ----

#[derive(Clone, Debug, PartialEq)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(v) => v.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    fn s(&self) -> &str {
        match self {
            J::Str(s) => s,
            o => panic!("forventet tekst: {o:?}"),
        }
    }
    fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            o => panic!("forventet liste: {o:?}"),
        }
    }
    fn n(&self) -> i64 {
        match self {
            J::Num(n) => *n as i64,
            o => panic!("forventet tall: {o:?}"),
        }
    }
    fn b(&self) -> bool {
        match self {
            J::Bool(b) => *b,
            o => panic!("forventet bool: {o:?}"),
        }
    }
    fn field(&self, k: &str) -> &J {
        self.get(k)
            .unwrap_or_else(|| panic!("mangler {k}: {self:?}"))
    }
    fn strs(&self, k: &str) -> Vec<String> {
        self.field(k)
            .arr()
            .iter()
            .map(|x| x.s().to_string())
            .collect()
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) {
        self.ws();
        assert_eq!(
            self.s[self.i], c,
            "JSON: forventet {} ved {}",
            c as char, self.i
        );
        self.i += 1;
    }
    fn lit(&mut self, word: &str, v: J) -> J {
        assert!(self.s[self.i..].starts_with(word.as_bytes()));
        self.i += word.len();
        v
    }
    fn string(&mut self) -> String {
        self.eat(b'"');
        let mut out = Vec::new();
        loop {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).unwrap(),
                b'\\' => {
                    let e = self.s[self.i];
                    self.i += 1;
                    out.push(match e {
                        b'n' => b'\n',
                        b't' => b'\t',
                        other => other,
                    });
                }
                c => out.push(c),
            }
        }
    }
    fn value(&mut self) -> J {
        self.ws();
        match self.s[self.i] {
            b'{' => {
                self.eat(b'{');
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        return J::Obj(v);
                    }
                    if !v.is_empty() {
                        self.eat(b',');
                    }
                    let k = self.string();
                    self.eat(b':');
                    v.push((k, self.value()));
                }
            }
            b'[' => {
                self.eat(b'[');
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        return J::Arr(v);
                    }
                    if !v.is_empty() {
                        self.eat(b',');
                    }
                    v.push(self.value());
                }
            }
            b'"' => J::Str(self.string()),
            b't' => self.lit("true", J::Bool(true)),
            b'f' => self.lit("false", J::Bool(false)),
            b'n' => self.lit("null", J::Null),
            _ => {
                let st = self.i;
                while self.i < self.s.len() && b"+-.0123456789eE".contains(&self.s[self.i]) {
                    self.i += 1;
                }
                J::Num(
                    std::str::from_utf8(&self.s[st..self.i])
                        .unwrap()
                        .parse()
                        .unwrap(),
                )
            }
        }
    }
}

fn fixture() -> J {
    P {
        s: FIXTURE.as_bytes(),
        i: 0,
    }
    .value()
}

// ---- utgang ----

type Out = Result<(), (String, String)>;

fn norm(e: ApiError) -> (String, String) {
    match e {
        ApiError::AckRequired(c) => {
            let keys: Vec<&str> = c.iter().map(|c| c.key()).collect();
            ("ack_required".into(), keys.join(","))
        }
        ApiError::UnknownCard(_) => ("unknown_card".into(), String::new()),
        ApiError::Internal(c) if V1_ERROR_CODES.contains(&c.as_str()) => (c, String::new()),
        other => ("core_error".into(), format!("{other:?}")),
    }
}

fn out<T>(r: Result<T, ApiError>) -> Out {
    r.map(|_| ()).map_err(norm)
}

fn check(expect: &J, got: &Out, ctx: &str) {
    match expect {
        J::Str(s) if s == "ok" => assert_eq!(got, &Ok(()), "{ctx}"),
        J::Obj(_) => {
            let want_code = expect.field("code").s();
            let Err((code, detail)) = got else {
                panic!("{ctx}: forventet feil {want_code}, fikk ok");
            };
            assert_eq!(code, want_code, "{ctx}: {got:?}");
            if let Some(d) = expect.get("detail") {
                assert_eq!(detail, d.s(), "{ctx}: {got:?}");
            }
        }
        o => panic!("{ctx}: ukjent forventning {o:?}"),
    }
}

// ---- miljø ----

struct Env {
    f: Fjell,
    kind: SuggestionKind,
    cards: Vec<Card>,
    scope: String,
    keeper: Option<AssetId>,
    snap: Option<u64>,
    person_assets: u64,
    faces_total: u64,
}

fn kind_of(code: &str) -> SuggestionKind {
    SuggestionKind::ALL
        .into_iter()
        .find(|k| k.as_str() == code)
        .unwrap_or_else(|| panic!("ukjent type {code}"))
}

fn env(kind: &str, scope: &str) -> Env {
    let mut fx = Fx::new();
    fx.corpus();
    let base = scene(700);
    fx.uniq("arkiv/s1.jpg", base.clone());
    fx.uniq("arkiv/s2.jpg", shifted(&base, 8, 8, 80));
    fx.uniq("arkiv/s3.jpg", shifted(&base, 20, 8, 80));
    let faces = Arc::new(
        MemFaceAnalyzer::new()
            .with_faces(scene(100), vec![face(1.0)])
            .with_faces(scene(101), vec![face(0.99)])
            .with_faces(scene(102), vec![face(0.0)]),
    );
    let f = Fjell::with_analyzers(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
        Arc::new(crate::model::NullEyeAnalyzer),
        faces,
    );
    scan(&f);
    // Én aktiv regel med ett treff (PNG-bildet), som de seedede reglene i mocken.
    f.rule_save(RuleDraft {
        id: None,
        name: "PNG".into(),
        spec: RuleSpec {
            all_of: vec![RuleCondition::Mime("image/png".into())],
        },
    })
    .unwrap();
    let k = kind_of(kind);
    let cards = all(&f, k);
    Env {
        f,
        kind: k,
        cards,
        scope: scope.into(),
        keeper: None,
        snap: None,
        person_assets: 0,
        faces_total: 0,
    }
}

impl Env {
    fn scope_of(&self, card: usize) -> ProtectScope {
        let a = self.cards[card].asset;
        if self.scope == "asset" {
            return ProtectScope::Asset(a);
        }
        let p = source_id_of(&self.f, a);
        let dir = p.rfind('/').map_or(p.clone(), |i| p[..i].to_string());
        ProtectScope::Folder(match self.scope.as_str() {
            "folder" => dir,
            // Ikke hele mappenavnet: beskytter ikke (hele stikomponenter).
            "folder_truncated" => dir[..dir.len() - 1].to_string(),
            // Annen skrivemåte: beskytter ikke (tegn for tegn, som i kjernen).
            "folder_upper" => dir.to_uppercase(),
            s => panic!("ukjent omfang {s}"),
        })
    }
    fn listed(&self, card: usize) -> Option<Card> {
        all(&self.f, self.kind)
            .into_iter()
            .find(|c| c.id == self.cards[card].id)
    }
    fn rule_hits(&self, card: usize) -> u64 {
        let ReasonDetail::Rule(id) = self.cards[card].detail else {
            panic!("ikke et regelkort");
        };
        self.f
            .rules_list()
            .unwrap()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
            .hits
    }
}

fn spec_of(conds: &J) -> RuleSpec {
    RuleSpec {
        all_of: conds
            .arr()
            .iter()
            .map(|c| match c.field("kind").s() {
                "is_screenshot" => RuleCondition::IsScreenshot,
                "older_than_days" => RuleCondition::OlderThanDays(c.field("value").n() as u32),
                "mime" => RuleCondition::Mime(c.field("value").s().to_string()),
                "in_folder" => RuleCondition::InFolder(c.field("value").s().to_string()),
                k => panic!("fixturen bruker vilkåret {k}, legg det til her og i mocken"),
            })
            .collect(),
    }
}

fn run_steps(env: &mut Env, steps: &[J], ctx: &str) {
    for (n, st) in steps.iter().enumerate() {
        let ctx = format!("{ctx} / steg {n} ({})", st.field("op").s());
        let card = || st.field("card").n() as usize;
        match st.field("op").s() {
            "protect" => {
                let s = env.scope_of(card());
                if st.field("on").b() {
                    env.f.protect_set(s).unwrap();
                } else {
                    env.f.unprotect(s).unwrap();
                }
            }
            "expect_flag" => {
                let c = env
                    .listed(card())
                    .unwrap_or_else(|| panic!("{ctx}: kortet vises ikke"));
                assert_eq!(
                    c.protect_flags.0 & SCOPE_FLAG != 0,
                    st.field("flagged").b(),
                    "{ctx}"
                );
            }
            "decide" => {
                let d = if st.field("decision").s() == "trash" {
                    Decision::Trash
                } else {
                    Decision::Keep
                };
                let ids: Vec<(CardId, Decision)> = st
                    .field("cards")
                    .arr()
                    .iter()
                    .map(|i| (env.cards[i.n() as usize].id, d))
                    .collect();
                let confirmed: Vec<ConsequenceCode> = st
                    .strs("confirmed")
                    .iter()
                    .map(|k| {
                        ConsequenceCode::ALL
                            .into_iter()
                            .find(|c| c.key() == k)
                            .unwrap()
                    })
                    .collect();
                check(
                    st.field("expect"),
                    &out(env.f.decide_many_confirmed(ids, confirmed)),
                    &ctx,
                );
            }
            "expect_pending" => {
                assert_eq!(
                    env.listed(card()).is_some(),
                    st.field("pending").b(),
                    "{ctx}"
                );
            }
            "expect_member_flags" => {
                let c = &env.cards[st.field("protected_card").n() as usize];
                let cmp = env.f.series_compare(SeriesId(c.group.unwrap().0)).unwrap();
                for m in &cmp.members {
                    assert_eq!(
                        m.protect_flags.0 & SCOPE_FLAG != 0,
                        m.asset == c.asset,
                        "{ctx}: {m:?}"
                    );
                }
            }
            "protect_keeper" => {
                let g = env.cards[0].group.unwrap();
                let k = env.f.series_compare(SeriesId(g.0)).unwrap().keeper;
                env.f.protect_set(ProtectScope::Asset(k)).unwrap();
                env.keeper = Some(k);
            }
            "expect_keeper_flag" => {
                let g = env.cards[0].group.unwrap();
                let cmp = env.f.series_compare(SeriesId(g.0)).unwrap();
                let m = cmp.members.iter().find(|m| m.is_keeper).unwrap();
                assert_eq!(
                    m.protect_flags.0 & SCOPE_FLAG != 0,
                    st.field("flagged").b(),
                    "{ctx}"
                );
            }
            "snapshot_rule_hits" => env.snap = Some(env.rule_hits(card())),
            "expect_rule_hits_delta" => {
                let now = env.rule_hits(card()) as i64;
                assert_eq!(
                    now - env.snap.unwrap() as i64,
                    st.field("delta").n(),
                    "{ctx}"
                );
            }
            "expect_assets_unique_across_kinds" => {
                let mut seen = BTreeSet::new();
                for k in SuggestionKind::ALL {
                    for c in all(&env.f, k) {
                        assert!(seen.insert(c.asset), "{ctx}: asset i flere kort: {c:?}");
                    }
                }
            }
            // Wipe-scenarioet.
            "enable_faces" => {
                env.f.face_index_set(true).unwrap();
                scan(&env.f);
            }
            "expect_people_at_least" => {
                assert!(
                    env.f.people_list().unwrap().len() as i64 >= st.field("n").n(),
                    "{ctx}"
                );
            }
            "protect_person" => {
                let people = env.f.people_list().unwrap();
                let p = &people[st.field("index").n() as usize];
                env.person_assets = p.asset_count;
                env.faces_total = people.iter().map(|p| p.face_count).sum();
                check(
                    st.field("expect"),
                    &out(env.f.person_protect(p.id, true)),
                    &ctx,
                );
            }
            "expect_scope_counts" => {
                let prot = env.f.protections().unwrap();
                let count = |person: bool| {
                    prot.iter()
                        .filter(|e| matches!(e.scope, ProtectScope::Person(_)) == person)
                        .count() as i64
                };
                assert_eq!(count(true), st.field("person").n(), "{ctx}: {prot:?}");
                if st.get("asset_equals_person_assets").is_some() {
                    assert_eq!(count(false), env.person_assets as i64, "{ctx}: {prot:?}");
                }
            }
            "wipe" => {
                assert_eq!(st.field("expect").s(), "faces_total");
                assert_eq!(env.f.face_index_wipe().unwrap(), env.faces_total, "{ctx}");
            }
            "expect_people_count" => {
                assert_eq!(
                    env.f.people_list().unwrap().len() as i64,
                    st.field("n").n(),
                    "{ctx}"
                );
            }
            o => panic!("{ctx}: ukjent operasjon {o}"),
        }
    }
}

fn run_error(e: &J) {
    let ctx = format!("feil: {}", e.field("name").s());
    let env = env("exact_dup", "asset");
    let f = &env.f;
    let name = |e: &J| match e.get("name_len") {
        Some(n) => "x".repeat(n.n() as usize),
        None => e.field("rule_name").s().to_string(),
    };
    let got: Out = match e.field("op").s() {
        "protect_target" => {
            let t = e.field("target").s();
            let id = || {
                if t == "missing" {
                    MISSING
                } else {
                    panic!("ukjent mål {t}")
                }
            };
            let scope = match e.field("scope").s() {
                "folder" => ProtectScope::Folder(t.to_string()),
                "asset" => ProtectScope::Asset(AssetId(id())),
                "album" => ProtectScope::Album(id()),
                "person" => ProtectScope::Person(PersonId(id())),
                s => panic!("ukjent omfang {s}"),
            };
            out(f.protect_set(scope))
        }
        "decide_missing" => {
            out(f.decide_many_confirmed(vec![(CardId(MISSING), Decision::Trash)], vec![]))
        }
        "compare_missing_series" => out(f.series_compare(SeriesId(MISSING))),
        "save_rule" => out(f.rule_save(RuleDraft {
            id: None,
            name: name(e),
            spec: spec_of(e.field("conditions")),
        })),
        "preview_rule" => {
            let r = f.rule_preview(spec_of(e.field("conditions")));
            if let (Ok(p), Some(h)) = (&r, e.get("expect_hits")) {
                assert_eq!(p.hits as i64, h.n(), "{ctx}");
            }
            out(r)
        }
        "toggle_missing_rule" => out(f.rule_toggle(RuleId(MISSING), true)),
        "delete_missing_rule" => out(f.rule_delete(RuleId(MISSING))),
        "restore_missing_rule" => out(f.rule_restore(RuleId(MISSING))),
        "name_missing_person" => out(f.person_name(PersonId(MISSING), Some("x".into()))),
        o => panic!("{ctx}: ukjent operasjon {o}"),
    };
    check(e.field("expect"), &got, &ctx);
}

#[test]
fn core_matches_conformance_fixture() {
    let fx = fixture();
    assert_eq!(fx.field("version").n(), 1);
    assert_eq!(fx.field("scope_flag").n(), SCOPE_FLAG as i64);
    let kinds = fx.strs("kinds");
    let mut ran = 0;
    for t in fx.field("templates").arr() {
        let only: Vec<String> = t.get("kinds").map_or(kinds.clone(), |_| t.strs("kinds"));
        for kind in kinds.iter().filter(|k| only.contains(k)) {
            for scope in t.strs("scopes") {
                let ctx = format!("{} [{kind}, {scope}]", t.field("name").s());
                let mut e = env(kind, &scope);
                assert!(!e.cards.is_empty(), "{ctx}: ingen kort av typen");
                run_steps(&mut e, t.field("steps").arr(), &ctx);
                ran += 1;
            }
        }
    }
    assert!(ran >= 30, "for få scenarier kjørt: {ran}");
    let mut w = env("exact_dup", "asset");
    run_steps(&mut w, fx.field("wipe").arr(), "wipe");
    for e in fx.field("errors").arr() {
        run_error(e);
    }
}
