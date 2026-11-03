use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use tracing::{info, warn};

use foundry::common::{dirs, process};

const PREVIOUS: &str = "battle-cats-complete";
const PREVIOUS_BINARY: &str = "Battle Cats Complete";
const BINARY: &str = "Hagane";
const ATTEMPTS: u32 = 20;
const PAUSE: Duration = Duration::from_millis(100);

static OUTCOMES: OnceLock<Vec<Result<String, String>>> = OnceLock::new();

fn carry(old: &Path, new: &Path) -> Result<String, String> {
    let mut failure = String::new();

    for _ in 0..ATTEMPTS {
        match fs::rename(old, new) {
            Ok(()) => return Ok(format!("Application folder {} carried over to {}", old.display(), new.display())),
            Err(err) => {
                failure = err.to_string();
                thread::sleep(PAUSE);
            }
        }
    }

    Err(format!("Application folder {} could not be carried over to {}: {failure}", old.display(), new.display()))
}

fn rename_binary() -> Result<Option<PathBuf>, String> {
    let exe = env::current_exe().map_err(|err| format!("The running executable could not be resolved: {err}"))?;

    if exe.file_stem().is_none_or(|stem| stem != PREVIOUS_BINARY) {
        return Ok(None);
    }

    let target = exe.with_file_name(format!("{BINARY}{}", env::consts::EXE_SUFFIX));

    if target.exists() {
        return Err(format!("{} kept its name because {} already exists", exe.display(), target.display()));
    }

    fs::rename(&exe, &target).map_err(|err| format!("{} could not be renamed: {err}", exe.display()))?;

    Ok(Some(target))
}

pub(crate) fn run() {
    let mut outcomes = Vec::new();

    match rename_binary() {
        Ok(Some(target)) => match process::command(&target).args(env::args_os().skip(1)).spawn() {
            Ok(_) => std::process::exit(0),
            Err(err) => outcomes.push(Err(format!("{} could not be relaunched: {err}", target.display()))),
        },
        Ok(None) => {}
        Err(note) => outcomes.push(Err(note)),
    }

    outcomes.extend(dirs::relocations(PREVIOUS).into_iter().filter(|(old, new)| old.is_dir() && !new.exists()).map(|(old, new)| carry(&old, &new)));

    let _ = OUTCOMES.set(outcomes);
}

pub(crate) fn report() {
    for outcome in OUTCOMES.get().into_iter().flatten() {
        match outcome {
            Ok(note) => info!("{}", note),
            Err(note) => warn!("{}", note),
        }
    }
}
