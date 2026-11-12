//! Skyplassholdere: er originalen på disken? Plassholdere leses aldri (ingen nedlasting).
//!
//! Grensesnitt: `is_cloud_placeholder(attrs)` (ren bitsjekk, testbar overalt), `LocalProbe`
//! (injiserbar sjekk) og `default_probe()` (Windows: filattributter via std; ellers alltid lokal).
//! Windows-attributter: OFFLINE 0x1000, RECALL_ON_OPEN 0x40000, RECALL_ON_DATA_ACCESS 0x400000.

use std::fs::Metadata;
use std::path::Path;
use std::sync::Arc;

pub const ATTR_OFFLINE: u32 = 0x1000;
pub const ATTR_RECALL_ON_OPEN: u32 = 0x4_0000;
pub const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

/// Sann hvis attributtene sier at innholdet må hentes fra sky ved lesing.
pub fn is_cloud_placeholder(attrs: u32) -> bool {
    attrs & (ATTR_OFFLINE | ATTR_RECALL_ON_OPEN | ATTR_RECALL_ON_DATA_ACCESS) != 0
}

/// Avgjør `is_local` for en fil. Kalles med metadata som allerede er hentet (ingen ekstra I/O).
pub type LocalProbe = Arc<dyn Fn(&Path, &Metadata) -> bool + Send + Sync>;

#[cfg(windows)]
fn probe(_p: &Path, md: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    !is_cloud_placeholder(md.file_attributes())
}

#[cfg(not(windows))]
fn probe(_p: &Path, _md: &Metadata) -> bool {
    true
}

pub fn default_probe() -> LocalProbe {
    Arc::new(probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_bits() {
        assert!(!is_cloud_placeholder(0x20)); // ARCHIVE
        assert!(is_cloud_placeholder(0x20 | ATTR_RECALL_ON_DATA_ACCESS));
        assert!(is_cloud_placeholder(ATTR_OFFLINE));
        assert!(is_cloud_placeholder(ATTR_RECALL_ON_OPEN));
    }
}
