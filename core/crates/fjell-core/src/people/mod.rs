//! Modul `people` (L2). V1-gren `people`: personer fra ansiktsindeks (av som standard), navngiving, sammenslåing og beskyttelse. Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt:
//! - `index_asset(store, analyzer, asset, thumb, algo)`: kjører `FaceAnalyzer` og lagrer ansikter. Bare når
//!   `face_index` er på (ellers `PeopleError::Disabled` og analysatoren kalles aldri). `Unavailable` fra
//!   analysatoren blir aldri til «ingen ansikter».
//! - `cluster_load`/`cluster_plan`/`cluster_apply`: samme som `cluster_new_faces`, i tre trinn (kort lås, regning
//!   uten lås, én transaksjon) så skrive-låsen ikke holdes under regningen.
//! - `cluster_new_faces(store, params, now)`: tildeler ansikter uten person til eksisterende eller nye personer
//!   (`cluster::plan_assignments`). Rører aldri ansikter brukeren eller en tidligere kjøring allerede har plassert.
//! - `list`, `set_name`, `merge`/`unmerge`, `split`: personer slik UI leser dem. Sammenslåing og splitt er
//!   angrebare uten at rader fjernes (`person.merged_into`; en splitt angres ved å slå den nye personen tilbake).
//! - `wipe`: «Slett ansiktsindeks» (embeddings og personer; avledet data, eksplisitt personvernvalg, ikke angrebar).
//! - `MemFaceAnalyzer`: deterministisk analysator til tester.
//!
//! Beskyttelse av person (`ProtectScope::Person`, tabellen `protection`) eies av `protect`; `person_assets`
//! gir bildene den skal dekke, og `list` viser `protected`. En `wipe` gjør personbeskyttelse om til beskyttelse av hvert bilde, så ingen bilder mister den.
//! Appen foreslår grupperinger; den dømmer ikke hvem som er hvem. Embeddings forlater aldri enheten.
//! Eier hele mappen `src/people/` og `store/people.rs`; ingen andre grener rører dem.

pub mod cluster;
mod mem;

pub use cluster::Target;
pub use mem::MemFaceAnalyzer;

use crate::model::{AnalyzeError, AssetId, FaceAnalyzer, PersonId, Thumb};
use crate::store::{AssignSlot, Store, StoreError};
use std::collections::HashMap;

/// Lengste tillatte personnavn i tegn.
pub const MAX_NAME_CHARS: usize = 100;

/// Feil fra personmodulen.
#[derive(Debug)]
pub enum PeopleError {
    /// Ansiktsindeksen er av; ingenting analyseres eller lagres.
    Disabled,
    /// Analysatoren feilet eller er ikke tilgjengelig (ikke det samme som «ingen ansikter»).
    Analyze(AnalyzeError),
    Store(StoreError),
}

impl From<StoreError> for PeopleError {
    fn from(e: StoreError) -> Self {
        PeopleError::Store(e)
    }
}

/// Parametere for klynging. Terskelen er en startverdi som kalibreres mot golden-korpus når modellen er valgt
/// (docs/v1/V1-PLAN.md, åpne spørsmål).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeopleParams {
    /// Minste cosinuslikhet for å regnes som samme person.
    pub threshold: f32,
}

impl Default for PeopleParams {
    fn default() -> Self {
        PeopleParams { threshold: 0.6 }
    }
}

/// Resultat av en klyngekjøring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClusterStats {
    /// Ansikter tildelt en person.
    pub assigned: u64,
    /// Nye personer opprettet.
    pub new_people: u64,
}

/// Analyserer ett bilde og lagrer ansiktene. Returnerer antall nye ansikter (0 hvis bildet allerede er
/// analysert med `algo` eller nyere, eller har ingen ansikter).
pub fn index_asset(
    store: &mut Store,
    analyzer: &dyn FaceAnalyzer,
    asset: AssetId,
    thumb: &Thumb,
    algo: u32,
) -> Result<usize, PeopleError> {
    if !store.face_index_enabled()? {
        return Err(PeopleError::Disabled);
    }
    if store.asset_has_faces(asset, algo)? {
        return Ok(0);
    }
    let faces = analyzer.detect(thumb).map_err(PeopleError::Analyze)?;
    if faces.is_empty() {
        return Ok(0);
    }
    Ok(store.put_faces(asset, &faces, algo)?.len())
}

/// Grunnlag for en klyngekjøring, lest under kort lås: eksisterende sentre og ansikter uten person.
pub struct ClusterInput {
    existing: Vec<(PersonId, crate::model::Embedding)>,
    pending: Vec<(u64, crate::model::Embedding)>,
}

impl ClusterInput {
    /// Antall ansikter som venter på person.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// Trinn 1 (kort lås): leser ansikter og bygger sentre. `None` når indeksen er av eller ingenting venter.
pub fn cluster_load(store: &Store) -> Result<Option<ClusterInput>, StoreError> {
    if !store.face_index_enabled()? {
        return Ok(None);
    }
    let faces = store.faces_all()?;
    let mut roots: HashMap<PersonId, Option<PersonId>> = HashMap::new();
    // Ett senter per lagret person (ikke per rot): en sammenslått person kan ha flere ulike utseender.
    let mut sums: HashMap<PersonId, (PersonId, Vec<f32>)> = HashMap::new();
    let mut pending = Vec::new();
    for f in &faces {
        match f.person {
            Some(p) => {
                let root = match roots.get(&p) {
                    Some(r) => *r,
                    None => {
                        let r = store.person_root(p)?;
                        roots.insert(p, r);
                        r
                    }
                };
                if let Some(root) = root {
                    let (_, s) = sums.entry(p).or_insert_with(|| (root, Vec::new()));
                    if s.is_empty() {
                        *s = vec![0.0; f.embedding.0.len()];
                    }
                    if s.len() == f.embedding.0.len() {
                        let n = f.embedding.0.iter().map(|x| x * x).sum::<f32>().sqrt();
                        if n > 0.0 && n.is_finite() {
                            for (a, b) in s.iter_mut().zip(&f.embedding.0) {
                                *a += b / n;
                            }
                        }
                    }
                }
            }
            None => pending.push((f.id, f.embedding.clone())),
        }
    }
    if pending.is_empty() {
        return Ok(None);
    }
    let mut own: Vec<(PersonId, (PersonId, Vec<f32>))> = sums.into_iter().collect();
    own.sort_by_key(|(o, _)| o.0);
    let existing = own
        .into_iter()
        .map(|(_, (root, v))| (root, crate::model::Embedding(v)))
        .collect();
    Ok(Some(ClusterInput { existing, pending }))
}

/// Trinn 2 (ingen lås): regner ut planen. Ren funksjon, O(ventende x sentre).
pub fn cluster_plan(input: &ClusterInput, params: &PeopleParams) -> Vec<(u64, Target)> {
    cluster::plan_assignments(&input.existing, &input.pending, params.threshold)
}

/// Trinn 3 (kort lås): lagrer planen i én transaksjon. Ansikter planen ikke lenger passer (slettet i
/// mellomtiden) hoppes over.
pub fn cluster_apply(
    store: &mut Store,
    plan: &[(u64, Target)],
    now: i64,
) -> Result<ClusterStats, StoreError> {
    let slots: Vec<(u64, AssignSlot)> = plan
        .iter()
        .map(|(f, t)| {
            (
                *f,
                match t {
                    Target::Existing(p) => AssignSlot::Existing(*p),
                    Target::New(n) => AssignSlot::New(*n),
                },
            )
        })
        .collect();
    let (assigned, new_people) = store.faces_assign_batch(&slots, now)?;
    Ok(ClusterStats {
        assigned,
        new_people,
    })
}

/// Tildeler alle ansikter uten person. Gjør ingenting når indeksen er av. Eksisterende personer (også
/// sammenslåtte, løst til rot) er sentre; ansikter som allerede har en person flyttes aldri. Samlet kall for
/// tester og enkle kallere; pipeline bruker trinnene hver for seg så skrive-låsen ikke holdes under regningen.
pub fn cluster_new_faces(
    store: &mut Store,
    params: &PeopleParams,
    now: i64,
) -> Result<ClusterStats, StoreError> {
    let Some(input) = cluster_load(store)? else {
        return Ok(ClusterStats::default());
    };
    let plan = cluster_plan(&input, params);
    cluster_apply(store, &plan, now)
}

/// En rot-person slik UI leser den.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonSummary {
    pub id: PersonId,
    pub name: Option<String>,
    pub face_count: u64,
    pub asset_count: u64,
    /// Bilde med det beste ansiktet (til avatar).
    pub cover: Option<AssetId>,
    pub protected: bool,
}

/// Utfall av en sammenslåing. Ingenting skrives for annet enn `Merged`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Rotpersonen som ble stående.
    Merged(PersonId),
    /// Begge er allerede samme person.
    SamePerson,
    /// En av personene finnes ikke.
    Unknown,
}

/// Personer (bare rot-personer), flest ansikter først.
pub fn list(store: &Store) -> Result<Vec<PersonSummary>, StoreError> {
    Ok(store
        .people_rows()?
        .into_iter()
        .map(|r| PersonSummary {
            id: r.id,
            name: r.name,
            face_count: r.face_count,
            asset_count: r.asset_count,
            cover: r.cover,
            protected: r.protected,
        })
        .collect())
}

/// Navngir en person. Tomt navn fjerner navnet. `Ok(false)` for ukjent person eller for langt navn.
pub fn set_name(store: &mut Store, id: PersonId, name: Option<&str>) -> Result<bool, StoreError> {
    if name.is_some_and(|n| n.trim().chars().count() > MAX_NAME_CHARS) {
        return Ok(false);
    }
    store.person_name_set(id, name)
}

/// Slår `absorbed` inn i `target`. Angres med `unmerge(absorbed)`.
pub fn merge(
    store: &mut Store,
    target: PersonId,
    absorbed: PersonId,
) -> Result<MergeOutcome, StoreError> {
    let (Some(t), Some(a)) = (store.person_root(target)?, store.person_root(absorbed)?) else {
        return Ok(MergeOutcome::Unknown);
    };
    if t == a {
        return Ok(MergeOutcome::SamePerson);
    }
    Ok(store
        .person_merge(t, a)?
        .map_or(MergeOutcome::Unknown, MergeOutcome::Merged))
}

/// Angrer en sammenslåing.
pub fn unmerge(store: &mut Store, absorbed: PersonId) -> Result<bool, StoreError> {
    store.person_unmerge(absorbed)
}

/// Splitter de oppgitte ansiktene ut til en ny person. Angres med `merge(store, person, ny)`.
pub fn split(
    store: &mut Store,
    person: PersonId,
    faces: &[u64],
    now: i64,
) -> Result<Option<PersonId>, StoreError> {
    store.person_split(person, faces, now)
}

/// Aktive bilder med personen (grunnlag for `ProtectScope::Person` i `protect`).
pub fn person_assets(store: &Store, person: PersonId) -> Result<Vec<AssetId>, StoreError> {
    store.person_assets(person)
}

/// «Slett ansiktsindeks». Returnerer antall ansikter slettet.
pub fn wipe(store: &mut Store) -> Result<u64, StoreError> {
    store.face_index_wipe()
}

#[cfg(test)]
mod tests;
