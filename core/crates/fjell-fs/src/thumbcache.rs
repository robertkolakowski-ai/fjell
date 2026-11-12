//! Miniatyrbuffer på disk (ARKITEKTUR 21): `%LOCALAPPDATA%\Fjell\cache\thumbs`, begrenset i størrelse,
//! LRU-opprydding. Kan slettes uten tap (alt kan regenereres). Rører aldri originalfiler: skriver
//! bare under egen `dir`, og nøkler er hex så de ikke kan forlate mappen.
//!
//! Grensesnitt: `ThumbCache::new(dir, max_bytes)`, `default_dir()`, `get`, `put`, `evict`, `clear`.
//! Format: `w` og `h` (u32 LE) fulgt av RGBA8. Feil lengde regnes som bom og filen fjernes.
//! `put` skriver til temp og døper om (aldri halvferdig fil). Feil i bufferen gir `None`/`false`,
//! aldri feil for brukeren. Bare `Rgba8` bufres.

use fjell_core::model::{Thumb, ThumbFmt};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Standard grense (MB-størrelse for miniatyrer; visningsbildene har egen grense, standard 1 GB).
pub const DEFAULT_MAX_BYTES: u64 = 256 * 1024 * 1024;
const EXT: &str = "thumb";

pub struct ThumbCache {
    dir: PathBuf,
    max_bytes: u64,
}

/// `%LOCALAPPDATA%\Fjell\cache\thumbs`; `None` hvis miljøvariabelen mangler (ikke Windows).
pub fn default_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty())?;
    Some(
        PathBuf::from(base)
            .join("Fjell")
            .join("cache")
            .join("thumbs"),
    )
}

fn valid_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 128 && k.bytes().all(|b| b.is_ascii_hexdigit())
}

impl ThumbCache {
    pub fn new(dir: impl Into<PathBuf>, max_bytes: u64) -> ThumbCache {
        ThumbCache {
            dir: dir.into(),
            max_bytes,
        }
    }

    fn file(&self, key: &str) -> Option<PathBuf> {
        valid_key(key).then(|| self.dir.join(format!("{key}.{EXT}")))
    }

    pub fn get(&self, key: &str) -> Option<Thumb> {
        let p = self.file(key)?;
        let bytes = fs::read(&p).ok()?;
        let parsed = (|| {
            let w = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?);
            let h = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?);
            let n = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
            (w > 0 && h > 0 && bytes.len() == 8 + n).then(|| Thumb {
                w,
                h,
                fmt: ThumbFmt::Rgba8,
                data: bytes[8..].to_vec(),
            })
        })();
        match parsed {
            Some(t) => {
                // Marker som nylig brukt (LRU). Feil her er harmløs.
                if let Ok(f) = fs::OpenOptions::new().write(true).open(&p) {
                    let _ = f.set_modified(SystemTime::now());
                }
                Some(t)
            }
            None => {
                let _ = fs::remove_file(&p); // ødelagt buffer-fil, bare vår egen
                None
            }
        }
    }

    /// Lagrer og rydder opp. Returnerer sann hvis miniatyren ble lagret.
    pub fn put(&self, key: &str, t: &Thumb) -> bool {
        let Some(p) = self.file(key) else {
            return false;
        };
        if t.fmt != ThumbFmt::Rgba8 || t.data.len() != t.w as usize * t.h as usize * 4 {
            return false;
        }
        if 8 + t.data.len() as u64 > self.max_bytes || fs::create_dir_all(&self.dir).is_err() {
            return false;
        }
        let mut buf = Vec::with_capacity(8 + t.data.len());
        buf.extend_from_slice(&t.w.to_le_bytes());
        buf.extend_from_slice(&t.h.to_le_bytes());
        buf.extend_from_slice(&t.data);
        let tmp = self.dir.join(format!("{key}.tmp"));
        let ok = fs::write(&tmp, &buf).is_ok() && fs::rename(&tmp, &p).is_ok();
        if !ok {
            let _ = fs::remove_file(&tmp);
            return false;
        }
        self.evict();
        true
    }

    /// Sletter eldste buffer-filer til summen er under grensen. Rører bare `*.thumb` og `*.tmp` i `dir`.
    pub fn evict(&self) {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|x| x.to_str());
            if !matches!(ext, Some(EXT | "tmp")) {
                continue;
            }
            if let Ok(md) = e.metadata() {
                if md.is_file() {
                    files.push((md.modified().unwrap_or(SystemTime::UNIX_EPOCH), md.len(), p));
                }
            }
        }
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort(); // eldst først
        for (_, len, p) in files {
            if total <= self.max_bytes {
                break;
            }
            if fs::remove_file(&p).is_ok() {
                total -= len;
            }
        }
    }

    /// Tømmer bufferen (innstillinger). Originaler berøres ikke.
    pub fn clear(&self) {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if matches!(p.extension().and_then(|x| x.to_str()), Some(EXT | "tmp")) {
                let _ = fs::remove_file(p);
            }
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("fjell-tc-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn thumb(n: u32, v: u8) -> Thumb {
        Thumb {
            w: n,
            h: n,
            fmt: ThumbFmt::Rgba8,
            data: vec![v; (n * n * 4) as usize],
        }
    }

    #[test]
    fn roundtrip_and_bad_keys() {
        let c = ThumbCache::new(dir("rt"), 1 << 20);
        assert!(c.put("ab12", &thumb(4, 7)));
        assert_eq!(c.get("ab12"), Some(thumb(4, 7)));
        assert_eq!(c.get("cd34"), None);
        for bad in ["", "../x", "a/b", "zz", "a.b"] {
            assert!(!c.put(bad, &thumb(2, 1)), "{bad}");
            assert_eq!(c.get(bad), None);
        }
        let gray = Thumb {
            fmt: ThumbFmt::Gray8,
            data: vec![0; 4],
            ..thumb(2, 0)
        };
        assert!(!c.put("aa", &gray));
        let _ = fs::remove_dir_all(c.dir());
    }

    #[test]
    fn corrupt_file_is_miss_and_removed() {
        let c = ThumbCache::new(dir("bad"), 1 << 20);
        assert!(c.put("ab", &thumb(2, 1)));
        let p = c.dir().join("ab.thumb");
        fs::write(&p, [1, 2, 3]).unwrap();
        assert_eq!(c.get("ab"), None);
        assert!(!p.exists());
        let _ = fs::remove_dir_all(c.dir());
    }

    #[test]
    fn evicts_oldest_under_limit_and_get_refreshes() {
        // hver fil: 8 + 8*8*4 = 264 byte; grense rommer to
        let c = ThumbCache::new(dir("lru"), 600);
        assert!(c.put("01", &thumb(8, 1)));
        assert!(c.put("02", &thumb(8, 2)));
        let old = SystemTime::now() - Duration::from_secs(100);
        for (k, age) in [("01", 100), ("02", 50)] {
            let f = fs::OpenOptions::new()
                .write(true)
                .open(c.dir().join(format!("{k}.thumb")))
                .unwrap();
            f.set_modified(old + Duration::from_secs(100 - age))
                .unwrap();
        }
        assert!(c.get("01").is_some()); // 01 blir nyest
        assert!(c.put("03", &thumb(8, 3))); // sprenger grensen: 02 er eldst
        assert!(c.get("02").is_none());
        assert!(c.get("01").is_some() && c.get("03").is_some());
        // for stor enkeltfil lagres ikke
        assert!(!c.put("04", &thumb(32, 4)));
        c.clear();
        assert!(c.get("01").is_none());
        let _ = fs::remove_dir_all(c.dir());
    }
}
