//! Barneprosess for krasjtesten K7-6 (`tests/crash.rs`): `fjell-cli crash-child <scan|actions> <db> <n>`.
//! Foreldreprosessen dreper barnet med SIGKILL (`kill -9`) midt i skriving, åpner indeksen på nytt
//! (WAL gjenopprettes) og kontrollerer at ingen halv tilstand finnes. Barnet skriver én linje per
//! fullført steg til stdout (`ready`, `batch <n>`, `progress <n>`), så foreldren vet at skrivingen pågår.
//!
//! - `scan`: full pipeline over `n` syntetiske bilder med forsinket kilde, så skrivetråden jobber i flere sekunder.
//! - `actions`: løkke med `decide_many`(20 kort) og `undo` mot en ferdig skannet indeks.

use crate::perf::SynthSource;
use fjell_core::api::{Decision, Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{Clock, SuggestionKind};
use fjell_core::store::Store;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

struct Zero;
impl Clock for Zero {
    fn now(&self) -> i64 {
        1_700_000_000
    }
}

fn say(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{s}");
    let _ = o.flush();
}

/// Antall kort per `decide_many` i `actions`-modus (testen sjekker at hver batch har akkurat så mange rader).
pub const ACTION_BATCH: usize = 20;

/// Kjører barnet. Går til feil hvis den ikke blir drept.
pub fn child(mode: &str, db: &Path, n: usize) -> Result<(), String> {
    let store = Store::open(db).map_err(|e| format!("{e:?}"))?;
    match mode {
        "scan" => {
            let src = Arc::new(SynthSource::new(n).with_delay(Duration::from_micros(400)));
            let f = Fjell::new(store, src, Arc::new(Zero));
            say("ready");
            f.start_scan(ScanConfig {
                mode: ScanMode::Full,
                priority: 1,
                deadline: None,
            })
            .map_err(|e| format!("{e:?}"))?;
            // Fremdriften meldes slik at foreldren kan drepe midt i full fase.
            let mut last = 0;
            loop {
                let s = f.home_summary().scan;
                if s.done >= last + 25 {
                    last = s.done;
                    say(&format!("progress {}", s.done));
                }
                if matches!(s.phase, fjell_core::api::ScanPhase::Done) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            f.wait_scan();
            say("done");
        }
        "actions" => {
            let f = Fjell::new(store, Arc::new(SynthSource::new(n)), Arc::new(Zero));
            say("ready");
            for k in 0.. {
                let kind = SuggestionKind::ALL[k % 2];
                let page = f.cards(kind, 200, None).map_err(|e| format!("{e:?}"))?;
                let start = (k * ACTION_BATCH) % 100;
                let d: Vec<_> = page
                    .cards
                    .iter()
                    .skip(start)
                    .take(ACTION_BATCH)
                    .map(|c| (c.id, Decision::Trash))
                    .collect();
                if d.len() == ACTION_BATCH {
                    f.decide_many(d).map_err(|e| format!("{e:?}"))?;
                    say(&format!("batch {k}"));
                    f.undo().map_err(|e| format!("{e:?}"))?;
                }
                let _ = f.sync_groups();
            }
        }
        m => return Err(format!("ukjent modus {m}")),
    }
    Ok(())
}
