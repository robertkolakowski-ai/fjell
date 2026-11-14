//! Smoketest for `people` (kontraktskjelett). Grenen legger til egentlige tester her.

use crate::model::{Embedding, NullFaceAnalyzer, PersonId};

#[test]
fn smoke_person_ids_and_embedding_roundtrip() {
    assert_eq!(PersonId(2).0, 2);
    let e = Embedding(vec![0.5, -1.0]);
    assert_eq!(Embedding::from_le_bytes(&e.to_le_bytes()), Some(e));
    let _ = NullFaceAnalyzer;
}
