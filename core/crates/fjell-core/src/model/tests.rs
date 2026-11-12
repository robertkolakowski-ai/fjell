//! Enhetstester for `model`.

use super::*;

#[test]
fn hamming_counts_bits() {
    assert_eq!(Hash64(0).hamming(Hash64(u64::MAX)), 64);
    assert_eq!(Hash64(0b1011).hamming(Hash64(0b0001)), 2);
}

#[test]
fn state_roundtrip_and_terminal_values() {
    for s in [State::Active, State::Trashed, State::Purged] {
        assert_eq!(State::from_i64(s.as_i64()), Some(s));
    }
    assert_eq!(State::from_i64(3), None);
}

#[test]
fn reason_keys_unique_and_snake_case() {
    let mut keys: Vec<_> = ReasonCode::ALL.iter().map(|r| r.key()).collect();
    assert!(keys
        .iter()
        .all(|k| k.chars().all(|c| c.is_ascii_lowercase() || c == '_')));
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), ReasonCode::ALL.len());
}

#[test]
fn thumb_validation() {
    assert!(Thumb::new(2, 2, ThumbFmt::Gray8, vec![0; 4]).is_ok());
    assert!(Thumb::new(2, 2, ThumbFmt::Rgba8, vec![0; 16]).is_ok());
    assert_eq!(
        Thumb::new(0, 2, ThumbFmt::Gray8, vec![]),
        Err(ThumbError::ZeroSize)
    );
    assert_eq!(
        Thumb::new(257, 1, ThumbFmt::Gray8, vec![0; 257]),
        Err(ThumbError::TooLarge)
    );
    assert_eq!(
        Thumb::new(2, 2, ThumbFmt::Gray8, vec![0; 3]),
        Err(ThumbError::BadLength {
            expected: 4,
            got: 3
        })
    );
}

#[test]
fn protect_flags() {
    let f = ProtectFlags(ProtectFlags::FAVORITE | ProtectFlags::EDITED);
    assert!(f.has(ProtectFlags::FAVORITE) && f.has(ProtectFlags::EDITED));
    assert!(!f.has(ProtectFlags::SHARED_ALBUM));
    assert!(f.is_protected() && !ProtectFlags::default().is_protected());
}

#[test]
fn ticket_carries_confirmation() {
    let t = PurgeTicket::new(Confirmed::new(7), vec![AssetId(1)], 100);
    assert_eq!(
        (t.confirmed().digest(), t.ids(), t.issued_at()),
        (7, &[AssetId(1)][..], 100)
    );
}
