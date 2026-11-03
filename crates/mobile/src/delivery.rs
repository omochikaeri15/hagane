use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

#[cfg(any(target_os = "android", target_os = "ios"))]
use tracing::{info, warn};

use crate::pages::Context;
use crate::settings::Exports;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::picker;
use crate::{AppWindow, paths, platform};

const KEPT_HERE: &str = "Saved to the exports folder";
#[cfg(any(target_os = "android", target_os = "ios"))]
const SENT: &str = "Saved where you chose";
#[cfg(any(target_os = "android", target_os = "ios"))]
const PARTLY: &str = "The remaining files stayed in the exports folder";
#[cfg(any(target_os = "android", target_os = "ios"))]
const MIMES: [(&str, &str); 10] = [
    ("png", "image/png"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("jpg", "image/jpeg"),
    ("zip", "application/zip"),
    ("json", "application/json"),
    ("txt", "text/plain"),
    ("csv", "text/csv"),
    ("mp4", "video/mp4"),
    ("apk", "application/vnd.android.package-archive"),
];
#[cfg(any(target_os = "android", target_os = "ios"))]
const FALLBACK_MIME: &str = "application/octet-stream";

thread_local! {
    static DECLINED: RefCell<HashSet<PathBuf>> = RefCell::new(HashSet::new());
}

fn declined(path: &Path) -> bool {
    DECLINED.with(|held| held.borrow().contains(path))
}

fn decline(paths: impl IntoIterator<Item = PathBuf>) {
    DECLINED.with(|held| held.borrow_mut().extend(paths));
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).and_then(|meta| meta.modified()).unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn mime(path: &Path) -> &'static str {
    let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_ascii_lowercase();

    MIMES.iter().find(|(held, _)| *held == extension).map_or(FALLBACK_MIME, |(_, mime)| mime)
}

fn collect(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn prune(dir: &Path, root: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            prune(&path, root);

            if path != root && fs::read_dir(&path).is_ok_and(|mut left| left.next().is_none()) {
                let _ = fs::remove_dir(&path);
            }
        }
    }
}

pub fn announce(ui: &AppWindow, context: &Rc<Context>) {
    if context.state.exports() != Exports::Ask || !platform::SAVE_DIALOG {
        context.toast(ui, KEPT_HERE, false);
        return;
    }

    let root = Path::new(paths::EXPORTS);
    let mut found = Vec::new();

    collect(root, &mut found);
    found.retain(|path| !declined(path));
    found.sort_by_key(|path| modified(path));
    found.reverse();

    if found.is_empty() {
        context.toast(ui, KEPT_HERE, false);
        return;
    }

    offer(ui, context, found, 0);
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn offer(ui: &AppWindow, context: &Rc<Context>, mut left: Vec<PathBuf>, saved: usize) {
    let Some(path) = left.pop() else {
        prune(Path::new(paths::EXPORTS), Path::new(paths::EXPORTS));
        context.toast(ui, if saved > 0 { SENT } else { KEPT_HERE }, false);
        return;
    };

    picker::save(
        ui,
        context,
        path.clone(),
        Box::new(move |ui, ctx, done| {
            if done {
                info!(file = %path.display(), "Export saved through the system dialog");

                if let Err(err) = fs::remove_file(&path) {
                    warn!("The exported copy {} could not be removed: {err}", path.display());
                }

                offer(ui, ctx, left, saved + 1);
            } else {
                decline(left.into_iter().chain(std::iter::once(path)));
                prune(Path::new(paths::EXPORTS), Path::new(paths::EXPORTS));
                ctx.toast(ui, if saved > 0 { PARTLY } else { KEPT_HERE }, false);
            }
        }),
    );
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn offer(ui: &AppWindow, context: &Rc<Context>, left: Vec<PathBuf>, _saved: usize) {
    decline(left);
    prune(Path::new(paths::EXPORTS), Path::new(paths::EXPORTS));
    context.toast(ui, KEPT_HERE, false);
}
