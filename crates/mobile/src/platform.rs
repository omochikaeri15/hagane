use std::path::Path;
#[cfg(any(target_os = "android", target_os = "ios"))]
use std::path::PathBuf;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tracing::debug;
use tracing::warn;

#[cfg(target_os = "android")]
use emulator::java::{self, DisplayCutout};
use emulator::java::Activity;
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
pub fn open_folder(path: &Path) -> Result<(), String> {
    crate::android::open_folder(path)
}

#[cfg(target_os = "ios")]
pub fn open_folder(path: &Path) -> Result<(), String> {
    crate::ios::open_folder(path)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn open_folder(path: &Path) -> Result<(), String> {
    open::that(path).map_err(|err| err.to_string())
}

pub const ORIENTATION_ENFORCED: bool = cfg!(any(target_os = "android", target_os = "ios"));

#[cfg(target_os = "android")]
pub fn set_orientation(orientation: Orientation) {
    if let Err(err) = crate::android::set_orientation(orientation) {
        warn!("The screen orientation could not be changed: {err}");
    }
}

#[cfg(target_os = "ios")]
pub fn set_orientation(orientation: Orientation) {
    if let Err(err) = crate::ios::set_orientation(orientation) {
        warn!("The screen orientation could not be changed: {err}");
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn set_orientation(orientation: Orientation) {
    debug!(?orientation, "Screen orientation is only enforced on phones");
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
pub const SAVE_DIALOG: bool = cfg!(any(target_os = "android", target_os = "ios"));

#[cfg(target_os = "android")]
pub fn pick(_ui: &AppWindow, multiple: bool, request: i32) -> Result<(), String> {
    crate::android::pick(multiple, request)
}

#[cfg(target_os = "ios")]
pub fn pick(ui: &AppWindow, multiple: bool, request: i32) -> Result<(), String> {
    crate::ios::pick(ui, multiple, request)
}

#[cfg(target_os = "android")]
pub fn save(_ui: &AppWindow, path: &Path, mime: &str, request: i32) -> Result<(), String> {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("export");

    crate::android::save(name, mime, request)
}

#[cfg(target_os = "ios")]
pub fn save(ui: &AppWindow, path: &Path, _mime: &str, request: i32) -> Result<(), String> {
    crate::ios::save(ui, path, request)
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
pub fn write_document(_uri: &str, _path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "android")]
pub fn set_fullscreen(fullscreen: bool) {
    if let Err(err) = crate::android::set_bars(fullscreen) {
        warn!("The system bars could not be changed: {err}");
    }
}

#[cfg(target_os = "ios")]
pub fn set_fullscreen(fullscreen: bool) {
    if let Err(err) = crate::ios::set_fullscreen(fullscreen) {
        warn!("The system bars could not be changed: {err}");
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn set_fullscreen(fullscreen: bool) {
    debug!(fullscreen, "The system bars are only controlled on phones");
}

#[cfg(target_os = "android")]
pub fn on_shown(handler: impl Fn(bool) + 'static) {
    crate::android::on_shown(handler);
}

#[cfg(target_os = "ios")]
pub fn on_shown(handler: impl Fn(bool) + 'static) {
    crate::ios::on_shown(handler);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn on_shown(_handler: impl Fn(bool) + 'static) {}

#[cfg(target_os = "android")]
pub fn is_tablet() -> Option<bool> {
    activity().as_ref().map(java::is_tablet)
}

#[cfg(not(target_os = "android"))]
pub fn is_tablet() -> Option<bool> {
    None
}

#[cfg(target_os = "android")]
pub fn activity() -> Option<Activity> {
    let [sdk_int, multi_window, screen_width, screen_height, window_width, window_height, cut, left, top, right, bottom] =
        crate::android::screen_facts().inspect_err(|err| warn!("The screen could not be read: {err}")).ok()?;
    let mut activity = Activity {
        sdk_int,
        display_cutout: (cut != 0).then_some(DisplayCutout { left, top, right, bottom }),
        in_multi_window: multi_window != 0,
        display_size: (screen_width, screen_height),
        ..Activity::default()
    };
    let screen = (screen_width.max(screen_height), screen_width.min(screen_height));
    let window = (window_width.max(window_height), window_width.min(window_height));

    java::on_surface_changed(&mut activity, screen.0, screen.1);

    if window != screen {
        java::on_surface_changed(&mut activity, window.0, window.1);
    }

    Some(activity)
}

#[cfg(not(target_os = "android"))]
pub fn activity() -> Option<Activity> {
    None
}

#[cfg(target_os = "android")]
pub fn vibrate(strength: f32, millis: i32, _sharpness: f32) {
    if let Err(err) = crate::android::vibrate(strength, millis) {
        warn!("The phone could not vibrate: {err}");
    }
}

#[cfg(target_os = "ios")]
pub fn vibrate(strength: f32, millis: i32, sharpness: f32) {
    crate::ios::vibrate(strength, millis, sharpness);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn vibrate(strength: f32, millis: i32, sharpness: f32) {
    debug!(strength, millis, sharpness, "Vibration is only wired on phones");
}

#[cfg(target_os = "android")]
pub fn cancel_vibration() {
    if let Err(err) = crate::android::cancel_vibration() {
        warn!("The vibration could not be cancelled: {err}");
    }
}

#[cfg(target_os = "ios")]
pub fn cancel_vibration() {
    crate::ios::cancel_vibration();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn cancel_vibration() {}

#[cfg(target_os = "android")]
pub fn has_vibrator() -> bool {
    crate::android::has_vibrator().inspect_err(|err| warn!("The vibrator could not be queried: {err}")).unwrap_or(false)
}

#[cfg(target_os = "ios")]
pub fn has_vibrator() -> bool {
    crate::ios::has_vibrator()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
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
