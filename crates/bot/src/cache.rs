use std::collections::HashMap;
use std::env;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

const PARTIAL: &str = "partial";

pub(crate) struct Cache {
    dir: PathBuf,
    cap: u64,
    build: [u8; 32],
    held: Mutex<Held>,
}

#[derive(Default)]
struct Held {
    entries: HashMap<String, Entry>,
    total: u64,
}

struct Entry {
    size: u64,
    used: SystemTime,
}

pub(crate) struct Key(String);

pub(crate) struct Recipe(Sha256);

impl Recipe {
    pub(crate) fn add(&mut self, part: &[u8]) {
        self.0.update((part.len() as u64).to_le_bytes());
        self.0.update(part);
    }

    pub(crate) fn finish(self) -> Key {
        let digest = self.0.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);

        for byte in digest {
            let _ = write!(hex, "{byte:02x}");
        }

        Key(hex)
    }
}

impl Cache {
    pub(crate) fn open(dir: PathBuf, cap: u64) -> io::Result<Self> {
        let partial = dir.join(PARTIAL);

        if partial.exists() {
            fs::remove_dir_all(&partial)?;
        }

        fs::create_dir_all(&partial)?;

        let mut held = Held::default();

        for entry in fs::read_dir(&dir)?.flatten() {
            let Ok(meta) = entry.metadata() else { continue };

            if !meta.is_file() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            let used = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

            held.total += meta.len();
            held.entries.insert(name, Entry { size: meta.len(), used });
        }

        info!(files = held.entries.len(), megabytes = held.total / (1024 * 1024), cap_megabytes = cap / (1024 * 1024), "Cache opened");

        let cache = Self { dir, cap, build: build_hash(), held: Mutex::new(held) };

        cache.evict();

        Ok(cache)
    }

    pub(crate) fn recipe(&self) -> Recipe {
        let mut hasher = Sha256::new();

        hasher.update(self.build);

        Recipe(hasher)
    }

    pub(crate) fn get(&self, key: &Key, extension: &str) -> Option<Vec<u8>> {
        let name = file_name(key, extension);
        let path = self.dir.join(&name);
        let now = SystemTime::now();

        {
            let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
            let entry = held.entries.get_mut(&name)?;

            entry.used = now;
        }

        match fs::read(&path) {
            Ok(bytes) => {
                if let Err(err) = File::options().write(true).open(&path).and_then(|file| file.set_modified(now)) {
                    warn!(file = name, "A cache hit could not be marked as used: {}", err);
                }

                Some(bytes)
            }
            Err(err) => {
                warn!(file = name, "A cached file could not be read, it is made again: {}", err);
                self.forget(&name);
                None
            }
        }
    }

    pub(crate) fn scratch(&self, key: &Key, extension: &str) -> PathBuf {
        self.dir.join(PARTIAL).join(file_name(key, extension))
    }

    pub(crate) fn admit(&self, key: &Key, extension: &str, from: &Path) -> io::Result<Vec<u8>> {
        let name = file_name(key, extension);
        let path = self.dir.join(&name);

        fs::rename(from, &path)?;

        let bytes = fs::read(&path)?;

        {
            let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);

            if let Some(old) = held.entries.insert(name, Entry { size: bytes.len() as u64, used: SystemTime::now() }) {
                held.total = held.total.saturating_sub(old.size);
            }

            held.total += bytes.len() as u64;
        }

        self.evict();

        Ok(bytes)
    }

    pub(crate) fn keep(&self, key: &Key, extension: &str, bytes: &[u8]) -> io::Result<()> {
        let staged = self.scratch(key, extension);

        fs::write(&staged, bytes)?;
        self.admit(key, extension, &staged).map(|_| ())
    }

    fn forget(&self, name: &str) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(old) = held.entries.remove(name) {
            held.total = held.total.saturating_sub(old.size);
        }
    }

    fn evict(&self) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);

        while held.total > self.cap {
            let Some(oldest) = held.entries.iter().min_by_key(|(_, entry)| entry.used).map(|(name, _)| name.clone()) else {
                break;
            };

            if let Some(entry) = held.entries.remove(&oldest) {
                held.total = held.total.saturating_sub(entry.size);
            }

            if let Err(err) = fs::remove_file(self.dir.join(&oldest)) {
                error!(file = oldest, "An evicted cache file could not be deleted: {}", err);
            }
        }
    }
}

fn file_name(key: &Key, extension: &str) -> String {
    format!("{}.{}", key.0, extension)
}

fn build_hash() -> [u8; 32] {
    let bytes = env::current_exe().and_then(fs::read).unwrap_or_else(|err| {
        warn!("The running binary could not be read for the cache key, keys fall back to the version: {}", err);
        env!("CARGO_PKG_VERSION").as_bytes().to_vec()
    });

    Sha256::digest(bytes).into()
}
