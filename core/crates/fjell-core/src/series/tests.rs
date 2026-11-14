//! Smoketest for `series` (kontraktskjelett). Grenen legger til egentlige tester her.

use crate::model::{SeriesId, SuggestionKind};

#[test]
fn smoke_series_ids_and_kind() {
    assert_eq!(SeriesId(3).0, 3);
    assert_eq!(SuggestionKind::Series.as_str(), "series");
}
