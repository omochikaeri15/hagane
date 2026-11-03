use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::common::architecture;
use crate::common::job::{JobEvent, ProgressCounter};
use crate::domains::mining::Build;
use crate::domains::settings::ImportConfig;

use super::AdbImportType;
use super::engine;
use super::engine::keys;

pub const UPDATE_APK: &str = "split_InstallPack.apk";
const BASE_APK: &str = "base.apk";
const FILES: &str = "files";
const STAGING: &str = "/data/local/tmp/bcc_files";
const MODIFIED_APP: &str = "File modification suspected, do a clean install on device.";
const ABORTED: &str = "Job Aborted";

pub struct Installed {
    pub package: String,
    pub base: PathBuf,
    pub splits: Vec<PathBuf>,
    pub build: Option<Build>,
}

pub fn package_name(suffix: &str) -> String {
    format!("jp.co.ponos.battlecats{suffix}")
}

pub fn run(
    installed: Vec<Installed>,
    import_mode: AdbImportType,
    import_config: ImportConfig,
    emit: impl Fn(JobEvent) + Sync,
    abort_flag: &AtomicBool,
    progress: &ProgressCounter,
) -> Result<(), String> {
    let emit_log = |line: String| emit(JobEvent::Log(line));

    keys::verify(import_config.enforce_validation, &emit_log)?;

    if installed.is_empty() {
        return Err("The game is not installed on this device for the chosen region.".to_owned());
    }

    if import_mode == AdbImportType::All {
        emit_log("Checking Root Permissions...".to_owned());
        rooted()?;
        emit_log("Root access confirmed via su.".to_owned());
    }

    let _work = architecture::Scratch::claim();
    let repository = Path::new(architecture::WORK).join("import");
    let mut directories = Vec::new();
    let mut builds = Vec::new();

    for app in installed {
        if abort_flag.load(Ordering::Relaxed) {
            return Err(ABORTED.to_owned());
        }

        emit_log(format!("Pulling {}...", app.package));

        if let Some(build) = app.build {
            emit_log(format!("Found {} v{}", app.package, build.name));
            builds.push(build);
        }

        let target = repository.join(&app.package);

        match pull(&app.package, &app.base, &app.splits, &target, import_mode, import_config.ignore_modified_app, &emit_log) {
            Ok(()) => directories.push(target),
            Err(reason) => emit_log(format!("Skipping {}: {reason}", app.package)),
        }
    }

    if directories.is_empty() {
        return Err("No regions were successfully pulled.".to_owned());
    }

    if abort_flag.load(Ordering::Relaxed) {
        return Err(ABORTED.to_owned());
    }

    emit_log("Starting Processing Phase...".to_owned());

    engine::run_universal_import(&directories, import_config.structure, &emit, abort_flag, progress, builds).map_err(|reason| format!("Universal Import Failed: {reason}"))?;

    emit_log("All Operations Complete!".to_owned());

    Ok(())
}

fn rooted() -> Result<(), String> {
    let output = Command::new("su").args(["-c", "id"]).output().map_err(|err| format!("Root access is required to read the game's data folder, but su could not be run: {err}"))?;
    let identity = String::from_utf8_lossy(&output.stdout);

    if identity.contains("uid=0") {
        return Ok(());
    }

    Err("Root access is required to read the game's data folder. Pick Update Only on a device without root.".to_owned())
}

fn pull(package: &str, base: &Path, splits: &[PathBuf], target: &Path, import_mode: AdbImportType, ignore_modified_app: bool, emit_log: &dyn Fn(String)) -> Result<(), String> {
    if target.exists() {
        fs::remove_dir_all(target).map_err(|err| format!("the previous pull could not be cleared: {err}"))?;
    }

    fs::create_dir_all(target).map_err(|err| format!("the pull folder could not be created: {err}"))?;

    if import_mode == AdbImportType::All {
        copy_data_files(package, target)?;
    }

    if let Some(split) = splits.iter().find(|split| split.file_name().is_some_and(|name| name == UPDATE_APK)) {
        return copy_apk(split, &target.join(UPDATE_APK));
    }

    if !ignore_modified_app {
        return Err(MODIFIED_APP.to_owned());
    }

    emit_log(MODIFIED_APP.to_owned());
    emit_log("Importing the base app instead.".to_owned());
    copy_apk(base, &target.join(BASE_APK))
}

fn copy_apk(source: &Path, destination: &Path) -> Result<(), String> {
    let copied = fs::copy(source, destination).map_err(|err| format!("{} could not be read: {err}", source.display()))?;

    if copied == 0 {
        let _ = fs::remove_file(destination);

        return Err("APK verification failed after copy.".to_owned());
    }

    Ok(())
}

fn copy_data_files(package: &str, target: &Path) -> Result<(), String> {
    let source = format!("/data/data/{package}/{FILES}");
    let landing = target.join(FILES);
    let target_text = target.to_str().ok_or("the pull folder path is not valid text")?;
    let landing_text = landing.to_str().ok_or("the pull folder path is not valid text")?;
    let script = format!(
        "rm -rf '{STAGING}' && cp -r '{source}' '{STAGING}' && find '{STAGING}' -name '*:*' -delete; mkdir -p '{target_text}' && cp -r '{STAGING}' '{landing_text}' && chmod -R 777 '{landing_text}'; rm -rf '{STAGING}'"
    );
    let output = Command::new("su").args(["-c", &script]).output().map_err(|err| format!("Root Copy Failed: {err}"))?;

    if !output.status.success() {
        return Err(format!("Root Copy Failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }

    let pulled = fs::read_dir(&landing).map(Iterator::count).unwrap_or(0);

    if pulled == 0 {
        return Err("Pull verification failed: empty directory.".to_owned());
    }

    Ok(())
}
