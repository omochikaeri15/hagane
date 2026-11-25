use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};

use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::{debug, info, warn};

use crate::common::io::cache::{self, CacheSpec};
use crate::{Source, Vfs};

struct StashCache;

impl CacheSpec for StashCache {
    type Data = Vec<Kept>;
    const FILE: &'static str = "sandbox_cache.bin";
}

type Stamp = (u128, u64);

type Held = (Option<Stamp>, Arc<[u8]>);

#[derive(Serialize, Deserialize)]
struct Kept {
    path: PathBuf,
    stamp: Option<Stamp>,
    #[serde(serialize_with = "write_bytes", deserialize_with = "read_bytes")]
    bytes: Arc<[u8]>,
}

struct Blob;

impl Visitor<'_> for Blob {
    type Value = Arc<[u8]>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the bytes of a kept file")
    }

    fn visit_bytes<E: Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
        Ok(Arc::from(bytes))
    }
}

fn write_bytes<S: Serializer>(bytes: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(bytes)
}

fn read_bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Arc<[u8]>, D::Error> {
    deserializer.deserialize_bytes(Blob)
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();

    Some((modified, meta.len()))
}

pub fn purge() {
    cache::purge::<StashCache>();
}

pub struct Save {
    mark: u64,
    kept: Vec<Kept>,
}

impl Save {
    pub fn write(self) {
        let began = Instant::now();
        let Some(payload) = cache::encode::<StashCache>(self.mark, &self.kept) else {
            return;
        };

        if payload.len() as u64 > cache::SIZE_LIMIT {
            warn!(bytes = payload.len(), "The sandbox cache is past the size limit and was not written");

            return;
        }

        cache::store::<StashCache>(&payload);

        debug!(files = self.kept.len(), bytes = payload.len(), ms = began.elapsed().as_millis(), "The sandbox cache was written");
    }
}

#[derive(Default)]
pub struct Stash {
    held: FxHashMap<PathBuf, Held>,
    hint: Vec<PathBuf>,
    unwatched: bool,
    mark: Option<u64>,
    settled: bool,
    open: bool,
    keeping: bool,
    saved: bool,
    tried: bool,
}

impl Stash {
    pub fn unwatched() -> Self {
        Self { unwatched: true, ..Self::default() }
    }

    pub fn unwatch(&mut self) {
        self.unwatched = true;
    }

    pub fn stock(&mut self, vfs: &Vfs, settled: bool, files: &BTreeMap<Box<str>, Source>) {
        self.fill(cache::file_key(vfs.fingerprint()), settled, files, cache::read::<StashCache>);
    }

    fn fill(&mut self, mark: u64, settled: bool, files: &BTreeMap<Box<str>, Source>, stored: impl FnOnce() -> Option<(u64, Vec<Kept>)>) {
        if self.mark != Some(mark) {
            self.shed();
        }

        self.mark = Some(mark);
        self.settled = settled;
        self.open = true;

        if !self.held.is_empty() {
            self.recheck();

            return;
        }

        let began = Instant::now();

        if !mem::replace(&mut self.tried, true)
            && let Some((key, kept)) = stored()
        {
            if settled && key == mark {
                self.held = kept.into_iter().map(|kept| (kept.path, (kept.stamp, kept.bytes))).collect();
                self.saved = true;
                self.recheck();

                info!(files = self.held.len(), ms = began.elapsed().as_millis(), "The sandbox cache matches the file index and was adopted");

                return;
            }

            self.hint = kept.into_iter().map(|kept| kept.path).collect();
        }

        if self.hint.is_empty() {
            return;
        }

        let listed: HashSet<&Path> = files.values().filter(|source| !source.in_memory()).map(|source| source.path.as_path()).collect();
        let wanted: Vec<PathBuf> = mem::take(&mut self.hint).into_iter().filter(|path| listed.contains(path.as_path())).collect();

        let unwatched = self.unwatched;

        self.held = wanted
            .into_par_iter()
            .filter_map(|path| {
                let stamped = unwatched.then(|| stamp(&path)).flatten();
                let bytes = fs::read(&path).ok()?;

                Some((path, (stamped, Arc::from(bytes))))
            })
            .collect();
        self.saved = false;

        info!(files = self.held.len(), ms = began.elapsed().as_millis(), "The sandbox cache did not match the file index, so its files were read again");
    }

    fn recheck(&mut self) {
        if !self.unwatched {
            return;
        }

        let moved: Vec<PathBuf> = self
            .held
            .par_iter()
            .filter(|(path, (stamped, _))| stamped.is_none() || stamp(path) != *stamped)
            .map(|(path, _)| path.clone())
            .collect();

        if moved.is_empty() {
            return;
        }

        info!(files = moved.len(), "Files held for the sandbox changed on disk and will be read again");

        for path in moved {
            self.held.remove(&path);
        }

        self.saved = false;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.keeping = false;
    }

    pub fn forget(&mut self) {
        self.shed();
        self.mark = None;
    }

    fn shed(&mut self) {
        if !self.held.is_empty() {
            self.hint = mem::take(&mut self.held).into_keys().collect();
        }

        self.saved = false;
    }

    pub fn keep(&mut self) {
        self.keeping = self.open;
    }

    pub fn keeping(&self) -> bool {
        self.keeping
    }

    pub fn rest(&mut self) {
        self.keeping = false;
    }

    pub fn warm(&mut self, sources: &[&Source]) {
        if !self.keeping {
            return;
        }

        let unwatched = self.unwatched;
        let missing: Vec<&Source> = sources.iter().copied().filter(|source| !source.in_memory() && !self.held.contains_key(&source.path)).collect();

        if missing.len() < 2 {
            return;
        }

        let read: Vec<(PathBuf, Held)> = missing
            .into_par_iter()
            .filter_map(|source| {
                let stamped = unwatched.then(|| stamp(&source.path)).flatten();
                let bytes = source.read().ok()?;

                Some((source.path.clone(), (stamped, bytes)))
            })
            .collect();

        if read.is_empty() {
            return;
        }

        self.held.extend(read);
        self.saved = false;
    }

    pub fn settle(&mut self) -> Option<Save> {
        if !mem::take(&mut self.keeping) || self.saved || !self.settled || self.held.is_empty() {
            return None;
        }

        let mark = self.mark?;
        let kept = self.held.iter().map(|(path, (stamp, bytes))| Kept { path: path.clone(), stamp: *stamp, bytes: Arc::clone(bytes) }).collect();

        self.saved = true;

        Some(Save { mark, kept })
    }

    pub fn read(&mut self, source: &Source) -> io::Result<Arc<[u8]>> {
        if !self.open || source.in_memory() {
            return source.read();
        }

        if let Some((_, bytes)) = self.held.get(&source.path) {
            return Ok(Arc::clone(bytes));
        }

        let stamped = (self.keeping && self.unwatched).then(|| stamp(&source.path)).flatten();
        let bytes = source.read()?;

        if self.keeping {
            self.held.insert(source.path.clone(), (stamped, Arc::clone(&bytes)));
            self.saved = false;
        }

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!("hagane-stash-{name}-{}", std::process::id()));

            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("scratch folder");

            Self(root)
        }

        fn file(&self, name: &str, text: &str) -> Source {
            let path = self.0.join(name);

            fs::write(&path, text).expect("scratch file");

            Source::disk(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn listing(sources: &[&Source]) -> BTreeMap<Box<str>, Source> {
        sources.iter().map(|source| (Box::from(source.path.to_string_lossy().as_ref()), (*source).clone())).collect()
    }

    fn written(save: Save) -> (u64, Vec<Kept>) {
        let payload = cache::encode::<StashCache>(save.mark, &save.kept).expect("encodes");

        cache::decode::<StashCache>(&payload).expect("decodes")
    }

    #[test]
    fn kept_files_come_back_byte_for_byte() {
        // The engine is handed these bytes in place of the file, so a round trip may not
        // lose one, and sound files are not text.
        let kept = vec![
            Kept { path: PathBuf::from("game/unit001.csv"), stamp: None, bytes: Arc::from(&b"1,2,3\r\n"[..]) },
            Kept { path: PathBuf::from("game/snd000.caf"), stamp: Some((u128::MAX, 4)), bytes: Arc::from(&[0u8, 0xff, 0x80, 0x0a][..]) },
            Kept { path: PathBuf::from("game/empty.csv"), stamp: None, bytes: Arc::from(&[][..]) },
        ];
        let (key, back) = written(Save { mark: 7, kept });

        assert_eq!(key, 7);
        assert_eq!(back[0].bytes.as_ref(), b"1,2,3\r\n");
        assert_eq!(back[1].bytes.as_ref(), [0u8, 0xff, 0x80, 0x0a]);
        assert!(back[2].bytes.is_empty());
        assert_eq!(back[1].path, PathBuf::from("game/snd000.caf"));
        assert_eq!(back[1].stamp, Some((u128::MAX, 4)));
    }

    #[test]
    fn a_cache_from_another_file_index_is_never_served() {
        // The whole point of the key: a cache written under one fingerprint only tells a
        // later load which files to read, and every byte comes from the disk again.
        let scratch = Scratch::new("drifted");
        let source = scratch.file("unit001.csv", "new");
        let files = listing(&[&source]);
        let stale = || Some((1, vec![Kept { path: source.path.clone(), stamp: None, bytes: Arc::from(&b"old"[..]) }]));
        let mut stash = Stash::default();

        stash.fill(2, true, &files, stale);

        assert_eq!(stash.read(&source).expect("read").as_ref(), b"new");
    }

    #[test]
    fn a_matching_cache_waits_for_the_file_index_to_be_checked() {
        // Before this launch's walk has finished the fingerprint is last launch's, so a
        // match proves nothing yet. Once it has, the cache stands in for the disk.
        let scratch = Scratch::new("unchecked");
        let source = scratch.file("unit001.csv", "disk");
        let files = listing(&[&source]);
        let cached = || Some((1, vec![Kept { path: source.path.clone(), stamp: None, bytes: Arc::from(&b"cache"[..]) }]));
        let mut early = Stash::default();
        let mut checked = Stash::default();

        early.fill(1, false, &files, cached);
        checked.fill(1, true, &files, cached);

        assert_eq!(early.read(&source).expect("read").as_ref(), b"disk");
        assert_eq!(checked.read(&source).expect("read").as_ref(), b"cache");

        early.keep();
        checked.keep();

        assert!(early.settle().is_none(), "nothing is written under an unchecked fingerprint");
        assert!(checked.settle().is_none(), "an adopted cache is already on disk");
    }

    #[test]
    fn a_moved_file_index_drops_what_was_held() {
        // Within a session the fingerprint moves when the watcher sees a change, and the
        // front end also forgets outright. Either way the next load reads the disk.
        let scratch = Scratch::new("moved");
        let source = scratch.file("unit001.csv", "old");
        let files = listing(&[&source]);
        let mut stash = Stash::default();

        stash.fill(1, true, &files, || None);
        stash.keep();
        assert_eq!(stash.read(&source).expect("read").as_ref(), b"old");

        scratch.file("unit001.csv", "new");
        assert_eq!(stash.read(&source).expect("held").as_ref(), b"old");

        stash.fill(2, true, &files, || None);
        assert_eq!(stash.read(&source).expect("read").as_ref(), b"new");

        scratch.file("unit001.csv", "newer");
        stash.forget();
        stash.fill(2, true, &files, || None);
        assert_eq!(stash.read(&source).expect("read").as_ref(), b"newer");
    }

    #[test]
    fn a_host_with_no_file_watcher_checks_every_file_on_disk() {
        // Mobile has nothing that tells the file index a file changed under it, so there
        // the fingerprint alone is not enough: each battle compares what it holds, and
        // what the cache file holds, against the disk before serving it.
        let scratch = Scratch::new("unwatched");
        let source = scratch.file("unit001.csv", "old");
        let files = listing(&[&source]);
        let mut stash = Stash::unwatched();

        stash.fill(1, true, &files, || None);
        stash.keep();
        assert_eq!(stash.read(&source).expect("read").as_ref(), b"old");

        let (_, kept) = written(stash.settle().expect("a first load is written"));

        scratch.file("unit001.csv", "longer");
        stash.fill(1, true, &files, || None);
        assert_eq!(stash.read(&source).expect("read").as_ref(), b"longer");

        let mut relaunched = Stash::unwatched();

        relaunched.fill(1, true, &files, || Some((1, kept)));
        assert_eq!(relaunched.read(&source).expect("read").as_ref(), b"longer");
    }

    #[test]
    fn warming_reads_ahead_only_while_the_game_data_loads() {
        // A table's regional copies are read together so a first load is not one file at
        // a time. Outside that load nothing may be read ahead or held.
        let scratch = Scratch::new("warming");
        let english = scratch.file("Unit_Explanation1_en.csv", "en");
        let french = scratch.file("Unit_Explanation1_fr.csv", "fr");
        let files = listing(&[&english, &french]);
        let mut stash = Stash::default();

        stash.fill(1, true, &files, || None);
        stash.warm(&[&english, &french]);
        scratch.file("Unit_Explanation1_fr.csv", "frais");
        assert_eq!(stash.read(&french).expect("read").as_ref(), b"frais");

        stash.keep();
        stash.warm(&[&english, &french]);
        scratch.file("Unit_Explanation1_fr.csv", "plus tard");
        assert_eq!(stash.read(&french).expect("held").as_ref(), b"frais");
        assert_eq!(stash.read(&english).expect("held").as_ref(), b"en");
    }

    #[test]
    fn a_closed_stash_always_reads_the_disk() {
        // A replay plays from its own files and never stocks the stash, so it must see
        // each file as it is right now even when a battle held the same path earlier.
        let scratch = Scratch::new("closed");
        let source = scratch.file("unit001.csv", "old");
        let mut stash = Stash::default();

        stash.fill(1, true, &listing(&[&source]), || None);
        stash.keep();
        assert_eq!(stash.read(&source).expect("read").as_ref(), b"old");

        scratch.file("unit001.csv", "new");
        stash.close();

        assert_eq!(stash.read(&source).expect("read").as_ref(), b"new");
    }

    #[test]
    fn only_the_game_data_load_adds_files_and_it_is_written_once() {
        // What is kept is what the engine reads while its game data loads, the same set
        // every battle. A stage's own files must not grow the cache with every stage.
        let scratch = Scratch::new("keeping");
        let table = scratch.file("unit001.csv", "table");
        let stage = scratch.file("stage00.csv", "stage");
        let files = listing(&[&table, &stage]);
        let mut stash = Stash::default();

        stash.fill(1, true, &files, || None);
        stash.keep();
        stash.read(&table).expect("read");

        let (key, kept) = written(stash.settle().expect("a first load is written"));

        stash.read(&stage).expect("read");

        assert_eq!(key, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, table.path);

        stash.fill(1, true, &files, || None);
        stash.keep();
        stash.read(&table).expect("read");

        assert!(stash.settle().is_none(), "an unchanged set is not written again");

        stash.forget();
        stash.fill(1, true, &files, || None);
        stash.keep();
        stash.rest();
        stash.keep();

        assert!(stash.settle().is_some(), "a write that was put off still happens on the next load");
    }
}
