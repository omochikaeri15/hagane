use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::{error, info};

use foundry::common::architecture;

use crate::platform;

pub const IMPORT: &str = "import";
pub const EXPORTS: &str = "exports";
const FOLDERS: [&str; 5] = [architecture::GAME, architecture::MODS, architecture::SANDBOX, IMPORT, EXPORTS];
const HOME_OVERRIDE: &str = "HAGANE_HOME";
const MOBILE_DIR: &str = "mobile";

pub struct Home {
    pub data: PathBuf,
    pub private: Option<PathBuf>,
}

impl Home {
    pub fn local() -> Option<Self> {
        if cfg!(target_os = "ios") {
            let home = env::var_os("HOME").map(PathBuf::from)?;

            return Some(Self { data: home.join("Documents"), private: None });
        }

        let data = env::var_os(HOME_OVERRIDE).map(PathBuf::from).or_else(|| foundry::common::dirs::data().map(|dir| dir.join(MOBILE_DIR)))?;

        Some(Self { data, private: None })
    }

    pub fn anchor(&self) -> bool {
        if let Some(private) = &self.private {
            point_private(private);
        }

        if let Err(err) = fs::create_dir_all(&self.data) {
            error!("The data folder {} could not be created: {}", self.data.display(), err);
            return false;
        }

        if let Err(err) = env::set_current_dir(&self.data) {
            error!("The working directory could not be anchored to {}: {}", self.data.display(), err);
            return false;
        }

        for folder in FOLDERS {
            if folder == IMPORT && platform::SYSTEM_PICKER {
                continue;
            }

            if let Err(err) = fs::create_dir_all(folder) {
                error!("The {folder} folder could not be created: {}", err);
            }
        }

        info!("Anchored the working directory to {}", self.data.display());

        true
    }
}

fn point_private(private: &Path) {
    let config = private.join("config");
    let old_cache = private.join("cache");
    let cache = private.parent().map_or_else(|| old_cache.clone(), |parent| parent.join("cache"));
    let state = private.join("state");
    let data = private.join("data");

    if cache != old_cache
        && old_cache.exists()
        && let Err(err) = fs::remove_dir_all(&old_cache)
    {
        error!("The old cache folder {} could not be removed: {}", old_cache.display(), err);
    }

    for dir in [&config, &cache, &state, &data] {
        if let Err(err) = fs::create_dir_all(dir) {
            error!("The private folder {} could not be created: {}", dir.display(), err);
        }
    }

    unsafe {
        env::set_var("HOME", private);
        env::set_var("XDG_CONFIG_HOME", &config);
        env::set_var("XDG_CACHE_HOME", &cache);
        env::set_var("XDG_STATE_HOME", &state);
        env::set_var("XDG_DATA_HOME", &data);
    }
}

pub fn import_dir() -> PathBuf {
    PathBuf::from(IMPORT)
}

pub fn exports_dir() -> PathBuf {
    PathBuf::from(EXPORTS)
}
