//! CLI-versjon av filsystemadapteren: adapteren bor i `fjell-fs` (M5); CLI bruker den uten
//! bildedekoding, så `thumbnail` er `Unavailable` og bredde/høyde 0 (uendret oppførsel, testene
//! bruker syntetiske filer).
//!
//! Grensesnitt: `FsSource::new(root)` + `impl AssetSource` (delegerer).

use fjell_core::model::{AssetSource, ChangeSet, ChangeToken, SourceError, SourceId, Thumb};
use std::io::Read;
use std::path::PathBuf;

pub struct FsSource(fjell_fs::FsSource);

impl FsSource {
    pub fn new(root: impl Into<PathBuf>) -> FsSource {
        FsSource(fjell_fs::FsSource::new(root).without_decoding())
    }

    /// Kobler fjerningsdeteksjon (T11) til indeksen i `db`: kjente id-er er indeksens aktive bilder.
    /// Feil ved lesing gir tom liste, altså ingen fjerninger (aldri gjetting).
    pub fn with_index(self, db: &std::path::Path) -> FsSource {
        let db = db.to_path_buf();
        FsSource(self.0.with_known_ids(Box::new(move || {
            fjell_core::store::Store::active_source_ids_at(&db).unwrap_or_default()
        })))
    }
}

impl AssetSource for FsSource {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        self.0.list_changes(since)
    }
    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        self.0.thumbnail(id)
    }
    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        self.0.read_bytes(id)
    }
}
