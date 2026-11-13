//! Filsystemadapter: `AssetSource` over en mappe (rekursivt). KUN LESING av originaler.
//!
//! Grensesnitt: `FsSource::new(root)` (+ `with_locality`) og `impl AssetSource`. `source_id` er
//! relativ sti med `/`. Originalfiler åpnes bare skrivebeskyttet; ingen skrivekall mot roten.
//! - Frakoblet/ulesbar rot gir `SourceError::Unavailable`, aldri en tom liste (ellers ville alt
//!   sett ut som slettet; ARKITEKTUR 21, T27).
//! - Karantenemappen `.fjell-karantene` hoppes over (ARKITEKTUR 20).
//! - Skyplassholdere: `is_local = false`; innholdet leses aldri (`Unavailable`).
//! - Skannlisten er `SCAN_FORMATS` (JPEG/PNG, HEIC/HEIF/AVIF, RAW, video). JPEG/PNG (hvis dekoding er
//!   på, standard): dimensjoner og miniatyr etter EXIF-orientering. Formater uten dekoder på
//!   plattformen (HEIC/RAW uten WIC, all video) har bredde/høyde 0 og `thumbnail` gir `Unavailable`
//!   (ikke feil); `read_bytes` virker, så de kan hashes. Ødelagt JPEG/PNG gir `Corrupt`.
//! - Flere røtter i ett bibliotek: se `roots` (`MultiSource` over én `FsSource` per rot).
//! - Inkrementelt token er `fs1:<maks mtime>`. Fjernede filer (T11) meldes ved inkrementell
//!   skanning som de kjente id-ene (`with_known_ids`: indeksens AKTIVE bilder, aldri de i papirkurven,
//!   siden de ligger i karantenemappen med vilje) som ikke finnes i en vellykket, full gjennomgang av
//!   mappen. Uten `with_known_ids` meldes ingen fjerninger. Frakoblet rot er `Unavailable`, aldri
//!   fjernet, og en tom gjennomgang når det finnes kjente filer regnes som frakoblet (tom
//!   monteringsmappe), også `Unavailable`. Ved `since = None` meldes ingen fjerninger.
//! - Miniatyrbuffer (`with_thumb_cache`): bare på disk under bufferens egen mappe (appens datamappe),
//!   aldri i eller under biblioteksroten (da ignoreres bufferen). Nøkkelen er hex av (rot, id, mtime,
//!   størrelse), så en endret fil gir ny nøkkel. Skyplassholdere og avslått dekoding når aldri bufferen.
//!   Feil i bufferen er alltid bom, aldri feil for brukeren. Originalfiler leses bare.

use crate::locality::{default_probe, LocalProbe};
use crate::path::{os_path, resolve, strip_extended};
use crate::quarantine::QUARANTINE_DIR;
use crate::thumb::{self, FrameDecoder};
use crate::thumbcache::ThumbCache;
use fjell_core::model::{
    AssetMeta, AssetSource, ChangeSet, ChangeToken, Kind, ProtectFlags, SourceError, SourceId,
    Thumb,
};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Henter id-ene til indeksens aktive bilder, til fjerningsdeteksjon.
pub type KnownIds = Box<dyn Fn() -> Vec<SourceId> + Send + Sync>;

pub struct FsSource {
    root: PathBuf,
    local: LocalProbe,
    decode: bool,
    known: Option<KnownIds>,
    cache: Option<ThumbCache>,
    /// Ekstra dekoder for formater `image`-craten ikke tar (WIC på Windows, ingen ellers).
    extra: Option<&'static dyn FrameDecoder>,
}

/// 128-bit nøkkel (to FNV-1a med ulike startverdier) som hex. Bare hex-tegn, så den aldri kan
/// forlate bufferens mappe (`ThumbCache` avviser alt annet).
fn cache_key(parts: &[&[u8]]) -> String {
    let mut a: u64 = 0xcbf2_9ce4_8422_2325;
    let mut b: u64 = 0x9e37_79b9_7f4a_7c15;
    for p in parts {
        // Lengdeprefiks så («ab», «c») og («a», «bc») gir ulike nøkler.
        for byte in (p.len() as u64).to_le_bytes().iter().chain(p.iter()) {
            a = (a ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
            b = (b ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3) ^ (b >> 29);
        }
    }
    format!("{a:016x}{b:016x}")
}

/// Skannlisten: (filendelse, MIME, type). Alt annet hoppes over ved skanning.
/// - JPEG/PNG dekodes overalt. HEIC/HEIF/AVIF og RAW dekodes bare der plattformen har kodek
///   (Windows via WIC); ellers er de synlige (telles, hashes, kan være eksakte duplikater), men
///   `thumbnail` gir `Unavailable` og de får ingen visuell analyse.
/// - Video dekodes aldri: telles og listes med størrelse, og kan bare bli eksakt duplikat (hash).
pub const SCAN_FORMATS: &[(&str, &str, Kind)] = &[
    ("jpg", "image/jpeg", Kind::Photo),
    ("jpeg", "image/jpeg", Kind::Photo),
    ("png", "image/png", Kind::Photo),
    ("heic", "image/heic", Kind::Photo),
    ("heif", "image/heif", Kind::Photo),
    ("avif", "image/avif", Kind::Photo),
    ("dng", "image/x-adobe-dng", Kind::Photo),
    ("cr2", "image/x-canon-cr2", Kind::Photo),
    ("cr3", "image/x-canon-cr3", Kind::Photo),
    ("nef", "image/x-nikon-nef", Kind::Photo),
    ("arw", "image/x-sony-arw", Kind::Photo),
    ("orf", "image/x-olympus-orf", Kind::Photo),
    ("rw2", "image/x-panasonic-rw2", Kind::Photo),
    ("raf", "image/x-fuji-raf", Kind::Photo),
    ("mp4", "video/mp4", Kind::Video),
    ("m4v", "video/x-m4v", Kind::Video),
    ("mov", "video/quicktime", Kind::Video),
    ("avi", "video/x-msvideo", Kind::Video),
    ("mkv", "video/x-matroska", Kind::Video),
    ("webm", "video/webm", Kind::Video),
    ("wmv", "video/x-ms-wmv", Kind::Video),
    ("3gp", "video/3gpp", Kind::Video),
];

/// MIME og type for en fil på skannlisten (`None` = ikke med i skanningen).
pub fn mime_and_kind(path: &Path) -> Option<(&'static str, Kind)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    SCAN_FORMATS
        .iter()
        .find(|(e, _, _)| *e == ext)
        .map(|(_, mime, kind)| (*mime, *kind))
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
            known: None,
            cache: None,
            extra: thumb::platform_decoder(),
        }
    }

    /// Roten denne kilden leser fra (som oppgitt).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Bytter den ekstra dekoderen (tester: fake-dekoder for HEIC/RAW; `None` = som Linux).
    pub fn with_frame_decoder(mut self, extra: Option<&'static dyn FrameDecoder>) -> FsSource {
        self.extra = extra;
        self
    }

    /// Gir adapteren indeksens kjente id-er, så fjernede filer også oppdages etter omstart.
    pub fn with_known_ids(mut self, f: KnownIds) -> FsSource {
        self.known = Some(f);
        self
    }

    /// Slår på miniatyrbuffer på disk. Bufferen må ligge utenfor biblioteket (appens datamappe):
    /// ligger `cache.dir()` i eller under roten, ignoreres den, så ingenting skrives i brukerens mapper.
    pub fn with_thumb_cache(mut self, cache: ThumbCache) -> FsSource {
        // Sammenlign både som oppgitt og kanonisert (symlenker, ulik skrivemåte), så vi heller dropper
        // bufferen enn risikerer å skrive i brukerens mapper.
        let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let inside = cache.dir().starts_with(&self.root)
            || canon(cache.dir()).starts_with(canon(&self.root));
        self.cache = (!inside).then_some(cache);
        self
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
            // Symlenker følges ikke (unngår duplikater, løkker og synkmapper utenfor biblioteket).
            if fs::symlink_metadata(os_path(&p)).is_ok_and(|m| m.file_type().is_symlink()) {
                continue;
            }
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
        // På Windows gir `read_dir` over en `\\?\`-sti oppføringer med samme prefiks, så begge
        // sider normaliseres til visningsform før roten strippes. Ellers ville id-en bli hele stien.
        let ps = PathBuf::from(strip_extended(&p.to_string_lossy()));
        let rs = PathBuf::from(strip_extended(&self.root.to_string_lossy()));
        let rel = ps.strip_prefix(&rs).unwrap_or(&ps);
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
            thumb::oriented_dims_with(&os_path(p), self.extra).unwrap_or((0, 0))
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

impl FsSource {
    /// Fjernede filer: kjente id-er som ikke er med i `current`. Tom gjennomgang med kjente filer er
    /// mistenkelig (tom monteringsmappe) og gir `Unavailable`.
    fn removed_since(
        &self,
        incremental: bool,
        current: &BTreeSet<SourceId>,
    ) -> Result<Vec<SourceId>, SourceError> {
        let (Some(f), true) = (&self.known, incremental) else {
            return Ok(Vec::new());
        };
        let known: BTreeSet<SourceId> = f().into_iter().collect();
        if current.is_empty() && !known.is_empty() {
            return Err(SourceError::Unavailable);
        }
        Ok(known.difference(current).cloned().collect())
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
        let mut current: BTreeSet<SourceId> = BTreeSet::new();
        for p in paths {
            current.insert(self.id_of(&p));
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
        let removed = self.removed_since(floor.is_some(), &current)?;
        Ok(ChangeSet {
            changed,
            removed,
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
        let key = self.cache.as_ref().map(|_| {
            let t = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .unwrap_or_default();
            cache_key(&[
                self.root.to_string_lossy().as_bytes(),
                id.0.as_bytes(),
                &t.as_nanos().to_le_bytes(),
                &md.len().to_le_bytes(),
            ])
        });
        if let (Some(c), Some(k)) = (&self.cache, &key) {
            if let Some(hit) = c.get(k) {
                return Ok(hit);
            }
        }
        let made = thumb::make_thumb_with(&os_path(&p), self.extra)?;
        if let (Some(c), Some(k)) = (&self.cache, &key) {
            let _ = c.put(k, &made);
        }
        Ok(made)
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
