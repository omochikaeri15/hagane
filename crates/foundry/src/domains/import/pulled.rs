use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::common::architecture;
use crate::common::job::{JobEvent, ProgressCounter};
use crate::domains::mining::Build;
use crate::domains::settings::ImportConfig;

use super::engine;
use super::engine::{apk, keys};

pub fn run(
    directories: &[PathBuf],
    import_config: ImportConfig,
    yielding: &[&str],
    emit: impl Fn(JobEvent) + Sync,
    abort_flag: &AtomicBool,
    progress: &ProgressCounter,
) -> Result<(), String> {
    let emit_log = |line: String| emit(JobEvent::Log(line));

    keys::verify(import_config.enforce_validation, &emit_log)?;

    let _work = architecture::Scratch::claim();

    engine::run_universal_import(directories, import_config.structure, &emit, abort_flag, progress, Vec::new(), yielding)
        .map_err(|reason| format!("Universal Import Failed: {reason}"))
}

pub fn builds(packages: &[PathBuf]) -> Vec<Build> {
    apk::builds(packages)
}
