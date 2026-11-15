//! Kjernen merker bilder brukeren har beskyttet med `SCOPE_FLAG` (bit 16) i `Card.protect_flags` og i serien,
//! slik at skjermene aldri forhåndsvelger dem. Plattformflaggene (1, 2, 4, 8) er uberørt.
//!
//! Grensesnitt: bare tester.

use super::super::fjell_v1::SCOPE_FLAG;
use super::*;

/// Kildestien (`source_id`) til et bilde, rett fra indeksen.
pub(super) fn source_id_of(f: &Fjell, a: AssetId) -> String {
    f.with_store(|s| {
        s.conn()
            .query_row(
                "SELECT source_id FROM asset WHERE id = ?1",
                [a.0 as i64],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
    })
}

#[test]
fn scope_protected_cards_and_series_members_carry_scope_flag() {
    let fx = corpus_with_series();
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
    );
    scan(&f);
    let flag = |c: &Card| c.protect_flags.0 & SCOPE_FLAG != 0;
    let series = all(&f, SuggestionKind::Series);
    assert!(series.iter().all(|c| !flag(c)), "ingen beskyttelse ennå");
    let sid = SeriesId(series[0].group.unwrap().0);
    let cmp = f.series_compare(sid).unwrap();
    assert!(cmp
        .members
        .iter()
        .all(|m| m.protect_flags.0 & SCOPE_FLAG == 0));

    // Bilde.
    let victim = series[0].clone();
    f.protect_set(ProtectScope::Asset(victim.asset)).unwrap();
    let after = all(&f, SuggestionKind::Series);
    assert!(flag(after.iter().find(|c| c.id == victim.id).unwrap()));
    assert!(after.iter().filter(|c| c.id != victim.id).all(|c| !flag(c)));
    let cmp = f.series_compare(sid).unwrap();
    for m in &cmp.members {
        assert_eq!(m.protect_flags.0 & SCOPE_FLAG != 0, m.asset == victim.asset);
    }
    // Å fjerne beskyttelsen fjerner flagget (angre).
    f.unprotect(ProtectScope::Asset(victim.asset)).unwrap();
    assert!(all(&f, SuggestionKind::Series).iter().all(|c| !flag(c)));

    // Mappe (flat rot: prefikset er kildestien).
    let path = source_id_of(&f, victim.asset);
    f.protect_set(ProtectScope::Folder(path)).unwrap();
    assert!(flag(
        all(&f, SuggestionKind::Series)
            .iter()
            .find(|c| c.id == victim.id)
            .unwrap()
    ));
    // Plattformflagg blandes aldri med V1-flagget.
    assert_eq!(SCOPE_FLAG & 0x0F, 0);
}

/// Mappebeskyttelse (V1-PLAN åpent spørsmål 5, 17 og 18): hele stikomponenter og tegn for tegn (store/små
/// bokstaver teller), og beskyttelsen følger ikke med når filen flyttes utenfor Fjell (beslutning 21: «borte + ny»).
/// Testen låser dagens oppførsel så en senere endring er et bevisst valg.
#[test]
fn folder_protection_is_a_prefix_and_does_not_follow_a_move() {
    use crate::model::SourceId;
    use crate::pipeline::tests::meta;
    let mut fx = Fx::new();
    fx.uniq("arkiv/a.jpg", scene(710));
    fx.uniq("arkivert/b.jpg", scene(711));
    fx.uniq("annet/c.jpg", scene(712));
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
    );
    scan(&f);
    let id_of = |path: &str| -> AssetId {
        f.with_store(|s| {
            AssetId(
                s.conn()
                    .query_row("SELECT id FROM asset WHERE source_id = ?1", [path], |r| {
                        r.get::<_, i64>(0)
                    })
                    .unwrap() as u64,
            )
        })
    };
    let protected = |a: AssetId| f.with_store(|s| !s.protected_by_scope(&[a]).unwrap().is_empty());
    f.protect_set(ProtectScope::Folder("arkiv".into())).unwrap();
    assert!(protected(id_of("arkiv/a.jpg")));
    assert!(
        !protected(id_of("arkivert/b.jpg")),
        "hele stikomponenter: «arkiv» beskytter ikke søskenmappen «arkivert»"
    );
    assert!(!protected(id_of("annet/c.jpg")));
    // Skrivemåten må være eksakt (kjent svakhet på Windows, se V1-PLAN): «ARKIV» beskytter ikke «arkiv/».
    f.unprotect(ProtectScope::Folder("arkiv".into())).unwrap();
    f.protect_set(ProtectScope::Folder("ARKIV".into())).unwrap();
    assert!(!protected(id_of("arkiv/a.jpg")));
    f.unprotect(ProtectScope::Folder("ARKIV".into())).unwrap();
    f.protect_set(ProtectScope::Folder("arkiv".into())).unwrap();
    assert!(protected(id_of("arkiv/a.jpg")));

    // Flytting utenfor Fjell: gammel sti er borte, ny sti er et nytt bilde uten beskyttelse.
    let (_, bytes) = fx.files.get("arkiv/a.jpg").cloned().unwrap();
    assert!(fx.src.inner.remove(&SourceId("arkiv/a.jpg".into())));
    fx.src
        .inner
        .put(meta("annet/a.jpg", bytes.len() as u64, "t1"), None, bytes);
    scan(&f);
    assert!(
        !protected(id_of("annet/a.jpg")),
        "beskyttelsen følger ikke med en flytting"
    );
    // Beskyttelsen står igjen som den var (brukeren ser den i Innstillinger og kan fjerne den): ingenting forsvinner stille.
    assert_eq!(f.protections().unwrap().len(), 1);
}
