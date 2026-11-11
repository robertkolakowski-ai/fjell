//! Enhetstester for skallets kommandologikk (kjører på alle plattformer, uten Tauri).

use crate::dto::DecisionDto;
use crate::Shell;

#[test]
fn state_reflects_seed_cards() {
    let s = Shell::mock().state();
    assert_eq!(s.backend, "mock");
    assert_eq!(s.per_kind.iter().map(|k| k.count).sum::<u64>(), 4);
    assert_eq!(s.trash_count, 0);
    assert_eq!(s.scan_phase, "idle");
}

#[test]
fn cards_unknown_kind_is_bad_request() {
    let e = Shell::mock().cards("nope", 10, None).unwrap_err();
    assert_eq!(e.code, "bad_request");
}

#[test]
fn cards_page_serializes_camel_case() {
    let p = Shell::mock().cards("exact_dup", 10, None).unwrap();
    assert_eq!(p.cards.len(), 2);
    assert_eq!(p.cards[0].reason, "ExactCopy");
}

#[test]
fn decide_then_undo_roundtrip() {
    let s = Shell::mock();
    let b = s.decide(&[3], DecisionDto::Trash).unwrap();
    assert_eq!(s.state().trash_count, 1);
    assert_eq!(s.undo().unwrap(), b);
    assert_eq!(s.state().trash_count, 0);
}

#[test]
fn undo_without_batch_gives_error_code() {
    assert_eq!(Shell::mock().undo().unwrap_err().code, "nothing_to_undo");
}

#[test]
fn unknown_card_gives_error_code() {
    let e = Shell::mock().decide(&[999], DecisionDto::Keep).unwrap_err();
    assert_eq!(e.code, "unknown_card");
}
