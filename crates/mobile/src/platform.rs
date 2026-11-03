use std::path::Path;
#[cfg(any(target_os = "android", target_os = "ios"))]
use std::path::PathBuf;

#[cfg(not(target_os = "android"))]
use tracing::debug;
use tracing::warn;

use foundry::domains::import::device::Installed;

#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::AppWindow;
use crate::settings::Orientation;

#[cfg(target_os = "android")]
pub fn open_url(url: &str) {
    if let Err(err) = crate::android::open_url(url) {
        warn!("The link {url} could not be opened: {err}");
    }
}

#[cfg(target_os = "ios")]
pub fn open_url(url: &str) {
    warn!("Opening links is not wired on iOS yet: {url}");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn open_url(url: &str) {
    if let Err(err) = open::that(url) {
        warn!("The link {url} could not be opened: {err}");
    }
}

#[cfg(target_os = "android")]
pub fn set_orientation(orientation: Orientation) {
    if let Err(err) = crate::android::set_orientation(orientation) {
        warn!("The screen orientation could not be changed: {err}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn set_orientation(orientation: Orientation) {
    debug!(?orientation, "Screen orientation is only enforced on Android");
}

#[cfg(target_os = "android")]
pub fn installed(package: &str) -> Result<Option<Installed>, String> {
    crate::android::installed(package)
}

#[cfg(not(target_os = "android"))]
pub fn installed(package: &str) -> Result<Option<Installed>, String> {
    Err(format!("Installed apps can only be read on Android, so {package} cannot be looked up here"))
}

pub const DEVICE_IMPORT: bool = cfg!(target_os = "android");

#[cfg(target_os = "android")]
pub fn publish(path: &Path, folder: &str, mime: &str) -> Result<String, String> {
    crate::android::publish(path, folder, mime)
}

#[cfg(not(target_os = "android"))]
pub fn publish(path: &Path, _folder: &str, _mime: &str) -> Result<String, String> {
    Err(format!("{} stays in the exports folder, only Android has a Downloads collection", path.display()))
}

#[cfg(target_os = "android")]
pub fn install(uri: &str) -> Result<(), String> {
    crate::android::install(uri)
}

#[cfg(not(target_os = "android"))]
pub fn install(uri: &str) -> Result<(), String> {
    Err(format!("{uri} cannot be installed from here, only Android installs packages in place"))
}

pub const DOWNLOADS: bool = cfg!(target_os = "android");
pub const SYSTEM_PICKER: bool = cfg!(any(target_os = "android", target_os = "ios"));
pub const SAVE_DIALOG: bool = cfg!(target_os = "android");

#[cfg(target_os = "android")]
pub fn pick(_ui: &AppWindow, multiple: bool, request: i32) -> Result<(), String> {
    crate::android::pick(multiple, request)
}

#[cfg(target_os = "ios")]
pub fn pick(ui: &AppWindow, multiple: bool, request: i32) -> Result<(), String> {
    crate::ios::pick(ui, multiple, request)
}

#[cfg(target_os = "android")]
pub fn save(name: &str, mime: &str, request: i32) -> Result<(), String> {
    crate::android::save(name, mime, request)
}

#[cfg(target_os = "ios")]
pub fn save(name: &str, _mime: &str, _request: i32) -> Result<(), String> {
    Err(format!("{name} stays in the exports folder, iOS has no save dialog wired"))
}

#[cfg(target_os = "android")]
pub fn copy_document(uri: &str, folder: &Path) -> Result<PathBuf, String> {
    crate::android::copy_document(uri, folder)
}

#[cfg(target_os = "ios")]
pub fn copy_document(uri: &str, folder: &Path) -> Result<PathBuf, String> {
    let source = Path::new(uri);
    let name = source.file_name().ok_or_else(|| format!("{uri} has no file name"))?;
    let target = folder.join(name);

    std::fs::copy(source, &target).map_err(|err| format!("{uri} could not be copied: {err}"))?;

    Ok(target)
}

#[cfg(target_os = "android")]
pub fn write_document(uri: &str, path: &Path) -> Result<(), String> {
    crate::android::write_document(uri, path)
}

#[cfg(target_os = "ios")]
pub fn write_document(uri: &str, _path: &Path) -> Result<(), String> {
    Err(format!("{uri} cannot be written, iOS has no save dialog wired"))
}

#[cfg(target_os = "android")]
pub fn immersive() {
    if let Err(err) = crate::android::immersive() {
        warn!("The system bars could not be hidden: {err}");
    }
}
