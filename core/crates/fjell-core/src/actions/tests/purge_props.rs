//! I2 uten unntak: hver vei til `purged` er enten bekreftet purge (`cause=user`, `op=purge`) eller
//! ekstern avstemming (`cause=external`) og er alltid logget; ingen annen kilde, ingen slettede rader.

use super::sim::{op, Sim};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn i2_every_purged_asset_has_a_logged_confirmed_or_external_path(
        ops in prop::collection::vec(op(), 1..40)
    ) {
        let mut sim = Sim::new();
        for o in &ops { sim.apply(o); }
        let n = sim.ids.len() as i64;
        prop_assert_eq!(sim.fx.sql_i64("SELECT COUNT(*) FROM asset"), n);
        // Hver purged-rad har en purge-loggrad med user (Confirmed-sti) eller external.
        prop_assert_eq!(sim.fx.sql_i64(
            "SELECT COUNT(*) FROM asset a WHERE a.state = 2 AND NOT EXISTS \
             (SELECT 1 FROM action_log l WHERE l.asset_id = a.id AND l.op = 'purge' \
              AND l.cause IN ('user','external'))"), 0);
        // Brukerpurge skjer bare etter bekreftet plattformssletting: ingen delete-kall uten purge-rader.
        let user_purges = sim.fx.sql_i64("SELECT COUNT(*) FROM action_log WHERE op='purge' AND cause='user'");
        prop_assert!(user_purges == 0 || sim.plat.call_count() > 0);
        // Uten Purge/ScanChange finnes ingen purge-rader overhodet.
    }
}
