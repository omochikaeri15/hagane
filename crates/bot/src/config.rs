use std::env;
use std::path::PathBuf;

const TOKEN: &str = "HAGANE_TOKEN";
const SOURCE: &str = "HAGANE_SOURCE";
const CACHE_GB: &str = "HAGANE_CACHE_GB";
const CACHE_DIR: &str = "HAGANE_CACHE_DIR";
const DEFAULT_SOURCE: &str = "https://hagane.cloud";
const DEFAULT_CACHE_GB: u64 = 25;
const DEFAULT_CACHE_DIR: &str = "cache";
const GIGABYTE: u64 = 1024 * 1024 * 1024;

pub(crate) struct Config {
    pub(crate) token: String,
    pub(crate) source: String,
    pub(crate) cache_dir: PathBuf,
    pub(crate) cache_bytes: u64,
}

impl Config {
    pub(crate) fn from_env() -> Result<Self, String> {
        let token = env::var(TOKEN).ok().filter(|token| !token.trim().is_empty()).ok_or_else(|| format!("{TOKEN} is not set"))?;
        let source = env::var(SOURCE).ok().filter(|link| !link.trim().is_empty()).unwrap_or_else(|| DEFAULT_SOURCE.to_owned());
        let cache_dir = env::var(CACHE_DIR).ok().filter(|dir| !dir.trim().is_empty()).map_or_else(|| PathBuf::from(DEFAULT_CACHE_DIR), PathBuf::from);
        let cache_gb = env::var(CACHE_GB).ok().map_or(Ok(DEFAULT_CACHE_GB), |text| text.trim().parse::<u64>().map_err(|_| format!("{CACHE_GB} holds '{text}', which is not a whole number of gigabytes")))?;

        Ok(Self { token: token.trim().to_owned(), source, cache_dir, cache_bytes: cache_gb * GIGABYTE })
    }
}
