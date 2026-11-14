//! Smoketest for `protect` (kontraktskjelett). Grenen legger til egentlige tester her.

use crate::model::{AssetId, ProtectScope};

#[test]
fn smoke_scope_coding_roundtrips() {
    let s = ProtectScope::Asset(AssetId(9));
    assert_eq!(
        ProtectScope::from_parts(s.scope_code(), &s.target()),
        Some(s)
    );
    assert_eq!(ProtectScope::from_parts("tag", "x"), None);
}
