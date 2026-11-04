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
pub fn set_fullscreen(fullscreen: bool) {
    if let Err(err) = crate::android::set_bars(fullscreen) {
        warn!("The system bars could not be changed: {err}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn set_fullscreen(fullscreen: bool) {
    debug!(fullscreen, "The system bars are only controlled on Android");
}

#[cfg(target_os = "android")]
pub fn on_shown(handler: impl Fn(bool) + 'static) {
    crate::android::on_shown(handler);
}

#[cfg(not(target_os = "android"))]
pub fn on_shown(_handler: impl Fn(bool) + 'static) {}

#[cfg(target_os = "android")]
const TABLET_WIDTH_DP: i32 = 600;

#[cfg(target_os = "android")]
pub fn is_tablet() -> Option<bool> {
    crate::android::smallest_width_dp()
        .inspect_err(|err| warn!("The screen size could not be read: {err}"))
        .ok()
        .filter(|width| *width > 0)
        .map(|width| width >= TABLET_WIDTH_DP)
}

#[cfg(not(target_os = "android"))]
pub fn is_tablet() -> Option<bool> {
    None
}

#[cfg(target_os = "android")]
pub fn vibrate(strength: f32, millis: i32) {
    if let Err(err) = crate::android::vibrate(strength, millis) {
        warn!("The phone could not vibrate: {err}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn vibrate(strength: f32, millis: i32) {
    debug!(strength, millis, "Vibration is only wired on Android");
}

#[cfg(target_os = "android")]
pub fn cancel_vibration() {
    if let Err(err) = crate::android::cancel_vibration() {
        warn!("The vibration could not be cancelled: {err}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn cancel_vibration() {}

#[cfg(target_os = "android")]
pub fn has_vibrator() -> bool {
    crate::android::has_vibrator().inspect_err(|err| warn!("The vibrator could not be queried: {err}")).unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
pub fn has_vibrator() -> bool {
    false
}

#[cfg(target_os = "android")]
pub fn hover_input() -> bool {
    crate::android::has_mouse().inspect_err(|err| warn!("The input devices could not be read: {err}")).unwrap_or(false)
}

#[cfg(target_os = "ios")]
pub fn hover_input() -> bool {
    false
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn hover_input() -> bool {
    true
}
