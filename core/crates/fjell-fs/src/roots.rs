//! Flere røtter i ett bibliotek (docs/mvp/FLERE-MAPPER.md): id-prefiks, kilde og karantene per rot.
//!
//! Grensesnitt: `PRIMARY_KEY`, `prefixed`, `split`, `check_roots`, `RootsError`, `MultiSource`
//! (`AssetSource` over én `FsSource` per rot), `RootSet` (én `FsQuarantine` per rot med ruting).
//!
//! Regler:
//! - Første rot har nøkkel 1 og id-er UTEN prefiks (nøyaktig som et énrots-bibliotek). Øvrige røtter
//!   har nøkkel ≥ 2 og id `/r<nøkkel>/<relativ sti>`. En relativ sti begynner aldri med `/` og har
//!   aldri tomme ledd, så prefikset kan ikke kollidere med en fil i første rot (heller ikke på
//!   Linux, der `:` er lovlig i filnavn). `path::resolve` avviser id-er som begynner med `/`, så en
//!   id fra en annen rot kan aldri løses mot feil rot ved en feil.
//! - Røtter kan ikke overlappe (lik, inni eller rundt en annen), sammenlignet kanonisert.
//! - Karantenen ligger i hver rot (`<rot>/.fjell-karantene`), så flytting er alltid `rename` på
//!   samme volum. Hver operasjon rutes på id-ens prefiks: en id fra rot A slås aldri opp i karantene B.
//! - Frakoblet rot: hoppes over i skanning (ingenting meldt endret eller fjernet for den); bare når
//!   ALLE røtter er frakoblet gir `list_changes` `Unavailable`.
//! - Endringstoken: `fs1:<s>` når bare første rot er med (uendret format), ellers
//!   `fsm1:<nøkkel>=<s>,…`. En rot uten verdi i tokenet listes fullt.

use crate::quarantine::{FsQuarantine, Quarantine, QuarantineError, Reconciled};
use crate::source::FsSource;
use fjell_core::model::{
    AssetId, AssetSource, BatchId, ChangeSet, ChangeToken, SourceError, SourceId, Thumb,
};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Nøkkelen til første rot (id-er uten prefiks).
pub const PRIMARY_KEY: u32 = 1;

/// Delt oppslag av indeksens aktive id-er (alle røtter, med prefiks).
pub type SharedKnown = Arc<dyn Fn() -> Vec<SourceId> + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootsError {
    /// Ingen røtter, eller første rot har ikke nøkkel 1.
    NoPrimary,
    /// Nøkkel 0, nøkkel 1 på en annen enn første rot, eller samme nøkkel to ganger.
    BadKey(u32),
    /// To røtter er samme mappe, eller den ene ligger inni den andre.
    Overlap { a: PathBuf, b: PathBuf },
}

impl std::fmt::Display for RootsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootsError::NoPrimary => write!(f, "biblioteket mangler første mappe"),
            RootsError::BadKey(k) => write!(f, "ugyldig eller duplisert mappenøkkel {k}"),
            RootsError::Overlap { a, b } => write!(
                f,
                "mappene overlapper: {} og {} (en mappe kan ikke ligge inni en annen)",
                a.display(),
                b.display()
            ),
        }
    }
}

/// Id for `rel` i roten `key` (uendret for første rot).
pub fn prefixed(key: u32, rel: &SourceId) -> SourceId {
    if key == PRIMARY_KEY {
        rel.clone()
    } else {
        SourceId(format!("/r{key}/{}", rel.0))
    }
}

/// Deler en id i (rotnøkkel, relativ sti). `None` for et prefiks som ikke er velformet.
pub fn split(id: &SourceId) -> Option<(u32, SourceId)> {
    let Some(rest) = id.0.strip_prefix('/') else {
        return Some((PRIMARY_KEY, id.clone()));
    };
    let (head, rel) = rest.split_once('/')?;
    let digits = head.strip_prefix('r')?;
    // Bare kanonisk skrivemåte («r2», ikke «r02» eller «r+2»), så én rot har ett prefiks.
    let key: u32 = digits.parse().ok()?;
    if key <= PRIMARY_KEY || digits != key.to_string() || rel.is_empty() {
        return None;
    }
    Some((key, SourceId(rel.to_string())))
}

/// Kanonisk form for sammenligning; frakoblet/ukjent mappe sammenlignes leksikalsk (uten `.`).
fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(crate::path::os_path(p)).unwrap_or_else(|_| p.components().collect())
}

/// Ligger `a` og `b` i hverandre (eller er de samme mappe)?
pub fn overlaps(a: &Path, b: &Path) -> bool {
    let (ca, cb) = (canon(a), canon(b));
    ca.starts_with(&cb) || cb.starts_with(&ca)
}

/// Validerer en rotliste: første rot har nøkkel 1, øvrige unike nøkler ≥ 2, ingen overlapp.
pub fn check_roots(roots: &[(u32, PathBuf)]) -> Result<(), RootsError> {
    match roots.first() {
        Some((PRIMARY_KEY, _)) => {}
        _ => return Err(RootsError::NoPrimary),
    }
    for (i, (key, path)) in roots.iter().enumerate() {
        if i > 0 && *key <= PRIMARY_KEY {
            return Err(RootsError::BadKey(*key));
        }
        for (other_key, other) in &roots[..i] {
            if other_key == key {
                return Err(RootsError::BadKey(*key));
            }
            if overlaps(other, path) {
                return Err(RootsError::Overlap {
                    a: other.clone(),
                    b: path.clone(),
                });
            }
        }
    }
    Ok(())
}

/// `AssetSource` over flere røtter. Kun lesing, som `FsSource`.
pub struct MultiSource {
    roots: Vec<(u32, FsSource)>,
    known: Option<SharedKnown>,
}

fn bad_token() -> SourceError {
    SourceError::Failed("bad change token".into())
}

fn parse_token(t: &ChangeToken) -> Result<BTreeMap<u32, i64>, SourceError> {
    let mut out = BTreeMap::new();
    if let Some(s) = t.0.strip_prefix("fs1:") {
        out.insert(PRIMARY_KEY, s.parse::<i64>().map_err(|_| bad_token())?);
    } else if let Some(s) = t.0.strip_prefix("fsm1:") {
        for part in s.split(',').filter(|p| !p.is_empty()) {
            let (k, v) = part.split_once('=').ok_or_else(bad_token)?;
            out.insert(
                k.parse::<u32>().map_err(|_| bad_token())?,
                v.parse::<i64>().map_err(|_| bad_token())?,
            );
        }
    } else {
        return Err(bad_token());
    }
    Ok(out)
}

fn format_token(vals: &BTreeMap<u32, i64>) -> ChangeToken {
    if vals.keys().all(|k| *k == PRIMARY_KEY) {
        let v = vals.get(&PRIMARY_KEY).copied().unwrap_or(0);
        return ChangeToken(format!("fs1:{v}"));
    }
    let parts: Vec<String> = vals.iter().map(|(k, v)| format!("{k}={v}")).collect();
    ChangeToken(format!("fsm1:{}", parts.join(",")))
}

impl MultiSource {
    /// `roots` i bibliotekets rekkefølge (første har nøkkel 1). Overlappende røtter avvises.
    pub fn new(roots: Vec<(u32, FsSource)>) -> Result<MultiSource, RootsError> {
        let paths: Vec<(u32, PathBuf)> = roots
            .iter()
            .map(|(k, s)| (*k, s.root().to_path_buf()))
            .collect();
        check_roots(&paths)?;
        Ok(MultiSource { roots, known: None })
    }

    /// Indeksens aktive id-er (alle røtter). Hver rot får sine egne (uten prefiks) til
    /// fjerningsdeteksjon; id-er i røtter som ikke lenger er med i biblioteket meldes fjernet.
    pub fn with_known_ids(mut self, known: SharedKnown) -> MultiSource {
        self.roots = self
            .roots
            .into_iter()
            .map(|(key, src)| {
                let all = known.clone();
                let own = Box::new(move || {
                    all()
                        .iter()
                        .filter_map(split)
                        .filter(|(k, _)| *k == key)
                        .map(|(_, rel)| rel)
                        .collect()
                });
                (key, src.with_known_ids(own))
            })
            .collect();
        self.known = Some(known);
        self
    }

    fn route(&self, id: &SourceId) -> Result<(&FsSource, SourceId), SourceError> {
        let (key, rel) = split(id).ok_or(SourceError::NotFound)?;
        self.roots
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, s)| (s, rel))
            .ok_or(SourceError::NotFound)
    }
}

impl AssetSource for MultiSource {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        let floors = since.map(parse_token).transpose()?;
        let mut next: BTreeMap<u32, i64> = BTreeMap::new();
        let (mut changed, mut removed) = (Vec::new(), Vec::new());
        let mut online = 0usize;
        for (key, src) in &self.roots {
            let floor = floors.as_ref().and_then(|f| f.get(key)).copied();
            let token = floor.map(|f| ChangeToken(format!("fs1:{f}")));
            let set = match src.list_changes(token.as_ref()) {
                Ok(s) => s,
                // Frakoblet rot: ingenting endret, ingenting fjernet, og tokenet står stille.
                Err(SourceError::Unavailable) => {
                    if let Some(f) = floor {
                        next.insert(*key, f);
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };
            online += 1;
            let secs = parse_token(&set.next_token)?
                .get(&PRIMARY_KEY)
                .copied()
                .unwrap_or(0);
            next.insert(*key, secs);
            changed.extend(set.changed.into_iter().map(|mut m| {
                m.source_id = prefixed(*key, &m.source_id);
                m
            }));
            removed.extend(set.removed.iter().map(|r| prefixed(*key, r)));
        }
        if online == 0 {
            return Err(SourceError::Unavailable);
        }
        // Rader fra røtter som er tatt ut av biblioteket forsvinner fra indeksen ved inkrementell
        // skanning (filene er urørt). Uformede id-er røres ikke.
        if let (Some(known), true) = (&self.known, since.is_some()) {
            removed.extend(known().into_iter().filter(|id| {
                split(id).is_some_and(|(k, _)| !self.roots.iter().any(|(rk, _)| *rk == k))
            }));
        }
        next.entry(PRIMARY_KEY).or_insert(0);
        Ok(ChangeSet {
            changed,
            removed,
            next_token: format_token(&next),
        })
    }

    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        let (src, rel) = self.route(id)?;
        src.thumbnail(&rel)
    }

    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        let (src, rel) = self.route(id)?;
        src.read_bytes(&rel)
    }
}

/// Én rot med sin egen karantene.
pub struct Root {
    pub key: u32,
    pub path: PathBuf,
    pub quarantine: Arc<FsQuarantine>,
}

/// Bibliotekets røtter med karantene per rot. All ruting skjer på id-ens rotprefiks.
pub struct RootSet {
    roots: Vec<Root>,
}

impl RootSet {
    /// Karantenen legges i hver rot (`<rot>/.fjell-karantene`). Overlappende røtter avvises.
    pub fn new(roots: &[(u32, PathBuf)]) -> Result<RootSet, RootsError> {
        check_roots(roots)?;
        Ok(RootSet {
            roots: roots
                .iter()
                .map(|(key, path)| Root {
                    key: *key,
                    path: path.clone(),
                    quarantine: Arc::new(FsQuarantine::in_library(path)),
                })
                .collect(),
        })
    }

    pub fn roots(&self) -> &[Root] {
        &self.roots
    }

    pub fn get(&self, key: u32) -> Option<&Root> {
        self.roots.iter().find(|r| r.key == key)
    }

    /// Roten id-en hører til, og stien relativt til den. `None` for ukjent rot eller uformet id.
    pub fn route(&self, id: &SourceId) -> Option<(&Root, SourceId)> {
        let (key, rel) = split(id)?;
        self.get(key).map(|r| (r, rel))
    }

    /// Grupperer på rot (rekkefølgen i hver gruppe bevares); `Err` er de som ikke kan rutes.
    #[allow(clippy::type_complexity)]
    fn group<'a>(
        &'a self,
        items: &[(AssetId, SourceId)],
    ) -> (Vec<(&'a Root, Vec<(AssetId, SourceId)>)>, Vec<AssetId>) {
        let mut per: Vec<(&Root, Vec<(AssetId, SourceId)>)> =
            self.roots.iter().map(|r| (r, Vec::new())).collect();
        let mut lost = Vec::new();
        for (id, sid) in items {
            match self.route(sid) {
                Some((root, rel)) => {
                    if let Some((_, v)) = per.iter_mut().find(|(r, _)| r.key == root.key) {
                        v.push((*id, rel));
                    }
                }
                None => lost.push(*id),
            }
        }
        per.retain(|(_, v)| !v.is_empty());
        (per, lost)
    }

    /// Flytter hver fil til karantenen i SIN rot. Ukjent rot gir `NotFound` (filen er urørt).
    pub fn stage(
        &self,
        batch: BatchId,
        items: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, Result<(), QuarantineError>)> {
        let (per, lost) = self.group(items);
        let mut out: Vec<_> = lost
            .into_iter()
            .map(|id| (id, Err(QuarantineError::NotFound)))
            .collect();
        for (root, part) in per {
            out.extend(root.quarantine.stage(batch, &part));
        }
        out
    }

    /// Flytter tilbake fra karantenen i roten indeksens sti (`items`) peker på, aldri en annen.
    /// Svaret er den nye id-en med rotprefiks.
    pub fn unstage(
        &self,
        items: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, Result<SourceId, QuarantineError>)> {
        let (per, lost) = self.group(items);
        let mut out: Vec<_> = lost
            .into_iter()
            .map(|id| (id, Err(QuarantineError::NotFound)))
            .collect();
        for (root, part) in per {
            let ids: Vec<AssetId> = part.iter().map(|p| p.0).collect();
            out.extend(
                root.quarantine
                    .unstage(&ids)
                    .into_iter()
                    .map(|(id, r)| (id, r.map(|rel| prefixed(root.key, &rel)))),
            );
        }
        out
    }

    /// Oppstartsavstemming per rot. Hver karantene får bare de `trashed` som hører til dens rot;
    /// `trashed` uten kjent rot svarer `Unavailable` (ingenting endret, ingenting markert borte).
    pub fn reconcile(&self, trashed: &[(AssetId, SourceId)]) -> Vec<(AssetId, Reconciled)> {
        let (per, lost) = self.group(trashed);
        let mut out: Vec<_> = lost
            .into_iter()
            .map(|id| (id, Reconciled::Unavailable))
            .collect();
        for root in &self.roots {
            let part = per
                .iter()
                .find(|(r, _)| r.key == root.key)
                .map(|(_, v)| v.as_slice())
                .unwrap_or(&[]);
            out.extend(
                root.quarantine
                    .reconcile(part)
                    .into_iter()
                    .map(|(id, r)| match r {
                        Reconciled::Restored(rel) => {
                            (id, Reconciled::Restored(prefixed(root.key, &rel)))
                        }
                        other => (id, other),
                    }),
            );
        }
        out
    }

    /// Alt som ligger i karantene: (rotnøkkel, id, opprinnelig sti med prefiks).
    pub fn staged(&self) -> Vec<(u32, AssetId, SourceId)> {
        self.roots
            .iter()
            .flat_map(|r| {
                r.quarantine
                    .staged()
                    .into_iter()
                    .map(|(id, rel)| (r.key, id, prefixed(r.key, &rel)))
            })
            .collect()
    }

    /// Som `FsQuarantine::locate_for`, i karantenen til roten `original` hører til.
    pub fn locate_for(&self, id: AssetId, original: &SourceId) -> Option<PathBuf> {
        let (root, rel) = self.route(original)?;
        root.quarantine.locate_for(id, &rel)
    }
}
