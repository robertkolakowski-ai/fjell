//! Filsystemadapter: `AssetSource` over en mappe (rekursivt). KUN LESING.
//!
//! Grensesnitt: `FsSource::new(root)` + `impl AssetSource`. `source_id` er relativ sti med `/`.
//! Originalfiler åpnes bare med `File::open` (skrivebeskyttet); adapteren har ingen skrivekall
//! mot rotmappen. Bevist av `tests/originals.rs`.
//! Begrensninger (MVP-CLI): ingen bildedekoding i kjernen eller CLI, så `thumbnail` gir
//! `Unavailable` og bredde/høyde er 0. Inkrementelt token er `fs1:<maks mtime>`; fjernede filer
//! meldes derfor bare ved full listing (`since = None`).

use fjell_core::model::{
    AssetMeta, AssetSource, ChangeSet, ChangeToken, Kind, ProtectFlags, SourceError, SourceId,
    Thumb,
};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct FsSource {
    root: PathBuf,
}

fn mime_and_kind(path: &Path) -> Option<(&'static str, Kind)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" => ("image/jpeg", Kind::Photo),
        "png" => ("image/png", Kind::Photo),
        "heic" => ("image/heic", Kind::Photo),
        "mp4" => ("video/mp4", Kind::Video),
        "mov" => ("video/quicktime", Kind::Video),
        _ => return None,
    })
}

fn io_err(e: std::io::Error) -> SourceError {
    match e.kind() {
        std::io::ErrorKind::NotFound => SourceError::NotFound,
        _ => SourceError::Failed(e.to_string()),
    }
}

impl FsSource {
    pub fn new(root: impl Into<PathBuf>) -> FsSource {
        FsSource { root: root.into() }
    }

    fn walk(&self, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), SourceError> {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .map_err(io_err)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                self.walk(&p, out)?;
            } else if mime_and_kind(&p).is_some() {
                out.push(p);
            }
        }
        Ok(())
    }

    fn id_of(&self, p: &Path) -> SourceId {
        let rel = p.strip_prefix(&self.root).unwrap_or(p);
        let parts: Vec<_> = rel.iter().map(|s| s.to_string_lossy()).collect();
        SourceId(parts.join("/"))
    }

    fn path_of(&self, id: &SourceId) -> Result<PathBuf, SourceError> {
        // Avvis stier som kan forlate roten.
        if id.0.split('/').any(|c| c == ".." || c.is_empty()) || id.0.starts_with('/') {
            return Err(SourceError::NotFound);
        }
        Ok(self.root.join(&id.0))
    }

    fn meta(&self, p: &Path) -> Result<(AssetMeta, i64), SourceError> {
        let md = fs::metadata(p).map_err(io_err)?;
        let (mime, kind) = mime_and_kind(p).ok_or(SourceError::Corrupt)?;
        let t = md.modified().map_err(io_err)?;
        let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = d.as_secs() as i64;
        let meta = AssetMeta {
            source_id: self.id_of(p),
            kind,
            created_at: secs,
            mod_token: format!("{}.{:09}-{}", d.as_secs(), d.subsec_nanos(), md.len()),
            width: 0,
            height: 0,
            bytes: md.len(),
            mime: mime.to_string(),
            platform_screenshot_hint: false,
            has_camera_exif: None,
            is_local: true,
            protect_flags: ProtectFlags::default(),
            albums: Vec::new(),
            backup: None,
        };
        Ok((meta, secs))
    }
}

impl AssetSource for FsSource {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        let floor = match since {
            None => None,
            Some(t) => Some(
                t.0.strip_prefix("fs1:")
                    .and_then(|s| s.parse::<i64>().ok())
                    .ok_or_else(|| SourceError::Failed("bad change token".into()))?,
            ),
        };
        let mut paths = Vec::new();
        self.walk(&self.root, &mut paths)?;
        let mut changed = Vec::new();
        let mut max = floor.unwrap_or(0);
        for p in paths {
            let (m, secs) = self.meta(&p)?;
            max = max.max(secs);
            if floor.is_none_or(|f| secs > f) {
                changed.push(m);
            }
        }
        Ok(ChangeSet {
            changed,
            removed: Vec::new(),
            next_token: ChangeToken(format!("fs1:{max}")),
        })
    }

    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        // Finnes filen? Ellers NotFound; dekoding er plattformens jobb (beslutning 6).
        fs::metadata(self.path_of(id)?).map_err(io_err)?;
        Err(SourceError::Unavailable)
    }

    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        let f = fs::File::open(self.path_of(id)?).map_err(io_err)?;
        Ok(Box::new(f))
    }
}
