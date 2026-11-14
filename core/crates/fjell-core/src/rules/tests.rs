//! Smoketest for `rules` (kontraktskjelett). Grenen legger til egentlige tester her.

use crate::model::{RuleCondition, RuleId, RuleSpec};

#[test]
fn smoke_empty_rule_matches_nothing_by_definition() {
    assert!(RuleSpec::default().is_empty());
    let spec = RuleSpec {
        all_of: vec![RuleCondition::IsScreenshot],
    };
    assert!(!spec.is_empty());
    assert_eq!(RuleId(1).0, 1);
}
