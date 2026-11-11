//! Kommandoer for `fjell-cli`: `scan` (mot `MemApi`-mocken) og `list`.
//!
//! Grensesnitt: `list(&dyn AssetSource, out)` og `scan(&dyn AssetSource, out)`.
//! `scan` bygger eksakte-duplikat-kort (BLAKE3 over filbytes, størrelse som forfilter) og mater
//! dem til `MemApi` via `FjellApi`-kontrakten; ekte kjerne erstatter mocken i M2. Ingen skriving.

use fjell_core::api::{Card, FjellApi, MemApi, ScanConfig, ScanMode};
use fjell_core::model::{
    AssetId, AssetSource, CardId, FakeClock, GroupId, ProtectFlags, ReasonCode, SourceError,
    SuggestionKind,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

fn werr(e: std::io::Error) -> SourceError {
    SourceError::Failed(e.to_string())
}

/// Skriver én linje per fil: `source_id<TAB>bytes<TAB>mime`.
pub fn list(src: &dyn AssetSource, out: &mut dyn Write) -> Result<usize, SourceError> {
    let cs = src.list_changes(None)?;
    for m in &cs.changed {
        writeln!(out, "{}\t{}\t{}", m.source_id.0, m.bytes, m.mime).map_err(werr)?;
    }
    Ok(cs.changed.len())
}

/// Skanner kilden mot mocken og skriver en oppsummering.
pub fn scan(src: &dyn AssetSource, out: &mut dyn Write) -> Result<(), SourceError> {
    let cs = src.list_changes(None)?;
    // Forfilter: bare filer med lik størrelse kan være eksakte duplikater.
    let mut by_size: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, m) in cs.changed.iter().enumerate() {
        by_size.entry(m.bytes).or_default().push(i);
    }
    let mut by_hash: BTreeMap<[u8; 32], Vec<usize>> = BTreeMap::new();
    for idxs in by_size.values().filter(|v| v.len() > 1) {
        for &i in idxs {
            let mut r = src.read_bytes(&cs.changed[i].source_id)?;
            let mut h = blake3::Hasher::new();
            std::io::copy(&mut r, &mut h).map_err(werr)?;
            by_hash.entry(*h.finalize().as_bytes()).or_default().push(i);
        }
    }
    let mut cards = Vec::new();
    for (g, idxs) in by_hash.values().filter(|v| v.len() > 1).enumerate() {
        let keeper = AssetId(idxs[0] as u64 + 1);
        for &i in &idxs[1..] {
            cards.push(Card {
                id: CardId(cards.len() as u64 + 1),
                kind: SuggestionKind::ExactDup,
                asset: AssetId(i as u64 + 1),
                group: Some(GroupId(g as u64 + 1)),
                keeper: Some(keeper),
                reason: ReasonCode::ExactCopy,
                confidence: 1.0,
                bytes: cs.changed[i].bytes,
                protect_flags: ProtectFlags::default(),
            });
        }
    }
    let api = MemApi::new(cards, Arc::new(FakeClock::new(0)));
    let cfg = ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    };
    api.start_scan(cfg)
        .map_err(|e| SourceError::Failed(format!("{e:?}")))?;
    let h = api.home_summary();
    writeln!(out, "files: {}", cs.changed.len()).map_err(werr)?;
    for k in &h.per_kind {
        writeln!(out, "{}: {} ({} bytes)", k.kind.as_str(), k.count, k.bytes).map_err(werr)?;
    }
    writeln!(out, "reclaimable_bytes: {}", h.reclaimable_bytes).map_err(werr)?;
    Ok(())
}
