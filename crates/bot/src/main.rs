mod cache;
mod config;
mod discord;
mod fuzzy;
mod grammar;
mod library;
mod lookup;
mod prefs;
mod prose;
mod render;
mod statblock;
mod talents;
mod whitelist;

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use tracing::error;
use tracing_subscriber::EnvFilter;

use cache::Cache;
use config::Config;

const DEFAULT_LOG: &str = "info";

fn main() -> ExitCode {
    tracing_subscriber::fmt().with_ansi(io::stdout().is_terminal()).with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG))).init();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(reason) => {
            error!("The bot is not configured: {}", reason);
            return ExitCode::FAILURE;
        }
    };
    let cache = match Cache::open(config.cache_dir.clone(), config.cache_bytes) {
        Ok(cache) => cache,
        Err(err) => {
            error!("The cache folder could not be opened: {}", err);
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            error!("The async runtime could not start: {}", err);
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(discord::run(config, cache)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("The Discord connection ended: {}", err);
            ExitCode::FAILURE
        }
    }
}
