//! Filsystemadapter: `AssetSource` over en mappe (rekursivt). KUN LESING av originaler.
//!
//! Grensesnitt: `FsSource::new(root)` (+ `with_locality`) og `impl AssetSource`. `source_id` er
//! relativ sti med `/`. Originalfiler åpnes bare skrivebeskyttet; ingen skrivekall mot roten.
//! - Frakoblet/ulesbar rot gir `SourceError::Unavailable`, aldri en tom liste (ellers ville alt
//!   sett ut som slettet; ARKITEKTUR 21, T27).
//! - Karantenemappen `.fjell-karantene` hoppes over (ARKITEKTUR 20).
//! - Skyplassholdere: `is_local = false`; innholdet leses aldri (`Unavailable`).
//! - JPEG/PNG (hvis dekoding er på, standard): dimensjoner og miniatyr etter EXIF-orientering; andre formater har bredde/høyde 0
//!   og `thumbnail` gir `Unavailable`. Ødelagt JPEG/PNG gir `Corrupt`.
//! - Inkrementelt token er `fs1:<maks mtime>`; fjernede filer meldes bare via kjernens diff mot
//!   full listing (`since = None`).

use crate::locality::{default_probe, LocalProbe};
use crate::path::{os_path, resolve};
use crate::quarantine::QUARANTINE_DIR;
use crate::thumb;
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
    local: LocalProbe,
    decode: bool,
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
        FsSource {
            root: root.into(),
            local: default_probe(),
            decode: true,
        }
    }

    /// Bytter sjekken for skyplassholdere (til tester og til plattformspesifikke varianter).
    pub fn with_locality(mut self, probe: LocalProbe) -> FsSource {
        self.local = probe;
        self
    }

    /// Slår av all bildedekoding (testsele/CLI): bredde/høyde 0 og `thumbnail` gir `Unavailable`.
    pub fn without_decoding(mut self) -> FsSource {
        self.decode = false;
        self
    }

    /// Roten er borte (frakoblet disk, nettverk nede) eller ikke en mappe.
    fn root_offline(&self) -> bool {
        !os_path(&self.root).is_dir()
    }

    /// Feil under skanning: er roten borte, er det `Unavailable`, ellers vanlig feil.
    fn scan_err(&self, e: std::io::Error) -> SourceError {
        if self.root_offline() {
            SourceError::Unavailable
        } else {
            io_err(e)
        }
    }

    fn walk(&self, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), SourceError> {
        let mut entries: Vec<PathBuf> = fs::read_dir(os_path(dir))
            .map_err(|e| self.scan_err(e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for p in entries {
            if os_path(&p).is_dir() {
                if p.file_name().is_some_and(|n| n == QUARANTINE_DIR) {
                    continue;
                }
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
        resolve(&self.root, id)
    }

    fn meta(&self, p: &Path) -> Result<(AssetMeta, i64), SourceError> {
        let md = fs::metadata(os_path(p)).map_err(io_err)?;
        let (mime, kind) = mime_and_kind(p).ok_or(SourceError::Corrupt)?;
        let t = md.modified().map_err(io_err)?;
        let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = d.as_secs() as i64;
        let is_local = (self.local)(p, &md);
        // Plassholdere dekodes aldri (ville lastet ned); andre formater har ingen dekoder.
        let (width, height) = if is_local && self.decode {
            thumb::oriented_dims(&os_path(p)).unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        let meta = AssetMeta {
            source_id: self.id_of(p),
            kind,
            created_at: secs,
            mod_token: format!("{}.{:09}-{}", d.as_secs(), d.subsec_nanos(), md.len()),
            width,
            height,
            bytes: md.len(),
            mime: mime.to_string(),
            platform_screenshot_hint: false,
            has_camera_exif: None,
            is_local,
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
        if self.root_offline() {
            return Err(SourceError::Unavailable);
        }
        let mut paths = Vec::new();
        self.walk(&self.root, &mut paths)?;
        let mut changed = Vec::new();
        let mut max = floor.unwrap_or(0);
        for p in paths {
            let (m, secs) = match self.meta(&p) {
                Ok(x) => x,
                // Forsvant mellom listing og stat: roten borte er `Unavailable`, ellers hopp over.
                Err(SourceError::NotFound) if !self.root_offline() => continue,
                Err(SourceError::NotFound) => return Err(SourceError::Unavailable),
                Err(e) => return Err(e),
            };
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
        let p = self.path_of(id)?;
        let md = fs::metadata(os_path(&p)).map_err(|e| self.scan_err(e))?;
        if !(self.local)(&p, &md) {
            return Err(SourceError::Unavailable);
        }
        if !self.decode {
            return Err(SourceError::Unavailable);
        }
        thumb::make_thumb(&os_path(&p))
    }

    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        let p = self.path_of(id)?;
        let md = fs::metadata(os_path(&p)).map_err(|e| self.scan_err(e))?;
        if !(self.local)(&p, &md) {
            return Err(SourceError::Unavailable);
        }
        let f = fs::File::open(os_path(&p)).map_err(io_err)?;
        Ok(Box::new(f))
    }
}
