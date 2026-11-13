//! Utregningen bak `storage`: ett strømmende pass over aktive rader.

use super::*;
use rusqlite::Connection;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

/// Rot for en sti. Indeksen har ingen rotkolonne ennå (én rot), så alt er rot 0. Eneste sted som
/// kjenner til dette: flere røtter betyr at denne funksjonen (og skjemaet) utvides, ikke resten.
fn root_of(_source_id: &str) -> u32 {
    0
}

pub(super) fn storage_kind(k: &str) -> StorageKind {
    match k {
        "photo" => StorageKind::Photo,
        "video" => StorageKind::Video,
        _ => StorageKind::Other,
    }
}

fn kind_slot(k: StorageKind) -> usize {
    match k {
        StorageKind::Photo => 0,
        StorageKind::Video => 1,
        StorageKind::Other => 2,
    }
}

fn big(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

#[derive(Default, Clone, Copy)]
pub(super) struct Acc {
    pub count: u64,
    pub bytes: u64,
}

impl Acc {
    pub fn add(&mut self, bytes: u64) {
        self.count = self.count.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
    }
}

/// Haugpost; større er bedre (størst først, så sti, så id lavest).
type Entry = (u64, Reverse<String>, Reverse<u64>, StorageKind);

pub(super) struct Active {
    pub total: Acc,
    pub kinds: [Acc; 3],
    pub roots: BTreeMap<u32, Acc>,
    pub folders: Vec<FolderUsage>,
    pub largest: Vec<FileUsage>,
}

/// Står en kandidat bedre an enn dårligste i haugen?
fn beats(bytes: u64, path: &str, id: u64, worst: &Entry) -> bool {
    (bytes, Reverse(path), Reverse(id)) > (worst.0, Reverse(worst.1 .0.as_str()), worst.2)
}

pub(super) fn scan_active(conn: &Connection, opts: &StorageOptions) -> Result<Active, StoreError> {
    let mut st =
        conn.prepare_cached("SELECT id, source_id, kind, bytes FROM asset WHERE state = 0")?;
    let mut rows = st.query([])?;
    let mut total = Acc::default();
    let mut kinds = [Acc::default(); 3];
    let mut roots: BTreeMap<u32, Acc> = BTreeMap::new();
    let mut folders: BTreeMap<u32, HashMap<String, (u32, Acc)>> = BTreeMap::new();
    let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();
    while let Some(r) = rows.next()? {
        let id = big(r.get(0)?);
        let path = r.get_ref(1)?.as_str().map_err(rusqlite::Error::from)?;
        let kind = storage_kind(r.get_ref(2)?.as_str().map_err(rusqlite::Error::from)?);
        let bytes = big(r.get(3)?);
        let root = root_of(path);
        total.add(bytes);
        kinds[kind_slot(kind)].add(bytes);
        roots.entry(root).or_default().add(bytes);
        if opts.folder_limit > 0 && opts.folder_depth > 0 {
            add_to_folders(
                folders.entry(root).or_default(),
                path,
                bytes,
                opts.folder_depth,
            );
        }
        if opts.file_limit > 0 {
            let better = heap.len() < opts.file_limit
                || heap.peek().is_some_and(|w| beats(bytes, path, id, &w.0));
            if better {
                if heap.len() == opts.file_limit {
                    heap.pop();
                }
                heap.push(Reverse((
                    bytes,
                    Reverse(path.to_string()),
                    Reverse(id),
                    kind,
                )));
            }
        }
    }
    Ok(Active {
        total,
        kinds,
        roots,
        folders: top_folders(folders, opts.folder_limit),
        largest: heap
            .into_sorted_vec() // stigende på Reverse = best først
            .into_iter()
            .map(
                |Reverse((bytes, Reverse(path), Reverse(id), kind))| FileUsage {
                    id: AssetId(id),
                    root: root_of(&path),
                    path,
                    bytes,
                    kind,
                },
            )
            .collect(),
    })
}

/// Legger filen til hver forelder opp til `depth` nivåer. Stien brukes som den er (kilden normaliserer
/// `source_id`); en ledende `/` gir ingen tom mappe.
fn add_to_folders(map: &mut HashMap<String, (u32, Acc)>, path: &str, bytes: u64, depth: usize) {
    let mut level = 0u32;
    for (i, _) in path.match_indices('/').filter(|(i, _)| *i > 0) {
        level += 1;
        if level as usize > depth {
            break;
        }
        let key = &path[..i];
        match map.get_mut(key) {
            Some((_, a)) => a.add(bytes),
            None => {
                let mut a = Acc::default();
                a.add(bytes);
                map.insert(key.to_string(), (level, a));
            }
        }
    }
}

fn top_folders(
    folders: BTreeMap<u32, HashMap<String, (u32, Acc)>>,
    limit: usize,
) -> Vec<FolderUsage> {
    let mut all: Vec<FolderUsage> = folders
        .into_iter()
        .flat_map(|(root, m)| {
            m.into_iter().map(move |(path, (depth, a))| FolderUsage {
                root,
                path,
                depth,
                count: a.count,
                bytes: a.bytes,
            })
        })
        .collect();
    all.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then(a.root.cmp(&b.root))
            .then_with(|| a.path.cmp(&b.path))
    });
    all.truncate(limit);
    all
}

/// Synlighetsregelen til kortene (`store::cards`): ventende forslag på aktivt bilde uten ventende forslag
/// med høyere prioritet. Konsistens med `Store::kind_totals` er testet.
const VISIBLE: &str = "s.status = 'pending' AND a.state = 0 AND NOT EXISTS (SELECT 1 FROM suggestion s2 \
     WHERE s2.asset_id = s.asset_id AND s2.status = 'pending' AND \
     (CASE s2.kind WHEN 'exact_dup' THEN 0 WHEN 'visual_dup' THEN 1 WHEN 'screenshot' THEN 2 ELSE 3 END) < \
     (CASE s.kind WHEN 'exact_dup' THEN 0 WHEN 'visual_dup' THEN 1 WHEN 'screenshot' THEN 2 ELSE 3 END))";

fn reclaimable(conn: &Connection) -> Result<Vec<Reclaimable>, StoreError> {
    let mut acc = [Acc::default(); 4];
    let sql = format!(
        "SELECT s.kind, a.bytes FROM suggestion s JOIN asset a ON a.id = s.asset_id WHERE {VISIBLE}"
    );
    let mut st = conn.prepare_cached(&sql)?;
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let k = r.get_ref(0)?.as_str().map_err(rusqlite::Error::from)?;
        if let Some(i) = SuggestionKind::ALL.iter().position(|x| x.as_str() == k) {
            acc[i].add(big(r.get(1)?));
        }
    }
    Ok(SuggestionKind::ALL
        .into_iter()
        .zip(acc)
        .map(|(kind, a)| Reclaimable {
            kind,
            count: a.count,
            bytes: a.bytes,
        })
        .collect())
}

fn trash(conn: &Connection) -> Result<Acc, StoreError> {
    let mut st = conn.prepare_cached("SELECT bytes FROM asset WHERE state = 1")?;
    let mut rows = st.query([])?;
    let mut a = Acc::default();
    while let Some(r) = rows.next()? {
        a.add(big(r.get(0)?));
    }
    Ok(a)
}

pub(super) fn summarize(
    conn: &Connection,
    opts: &StorageOptions,
) -> Result<StorageSummary, StoreError> {
    // Lesetransaksjon: alle tre pass ser samme øyeblikksbilde (WAL).
    let tx = conn.unchecked_transaction()?;
    let a = scan_active(&tx, opts)?;
    let rec = reclaimable(&tx)?;
    let t = trash(&tx)?;
    tx.finish()?;
    let reclaimable_total_bytes = rec.iter().fold(0u64, |s, r| s.saturating_add(r.bytes));
    Ok(StorageSummary {
        total_count: a.total.count,
        total_bytes: a.total.bytes,
        per_kind: StorageKind::ALL
            .into_iter()
            .map(|kind| KindUsage {
                kind,
                count: a.kinds[kind_slot(kind)].count,
                bytes: a.kinds[kind_slot(kind)].bytes,
            })
            .collect(),
        per_root: a
            .roots
            .into_iter()
            .map(|(root, x)| RootUsage {
                root,
                count: x.count,
                bytes: x.bytes,
            })
            .collect(),
        per_folder: a.folders,
        largest_files: a.largest,
        reclaimable_by_category: rec,
        reclaimable_total_bytes,
        trash_count: t.count,
        trash_bytes: t.bytes,
    })
}
