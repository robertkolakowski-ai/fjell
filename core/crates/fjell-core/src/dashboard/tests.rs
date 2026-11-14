//! Smoketest for `dashboard` (kontraktskjelett). Grenen legger til egentlige tester her.

use crate::model::DashboardData;

#[test]
fn smoke_dashboard_default_is_empty() {
    let d = DashboardData::default();
    assert_eq!(
        (d.active_count, d.active_bytes, d.per_kind.len()),
        (0, 0, 0)
    );
}
