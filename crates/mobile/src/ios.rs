#![cfg(target_os = "ios")]

use std::cell::RefCell;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::rc::autoreleasepool;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_core_haptics::{
    CHHapticDeviceCapability, CHHapticEngine, CHHapticEvent, CHHapticEventParameter, CHHapticEventParameterIDHapticIntensity, CHHapticEventParameterIDHapticSharpness, CHHapticEventTypeHapticContinuous,
    CHHapticEventTypeHapticTransient, CHHapticPattern, CHHapticPatternPlayer,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSOperationQueue, NSString, NSURL};
use objc2_ui_kit::{UIApplication, UIApplicationDidBecomeActiveNotification, UIApplicationWillResignActiveNotification, UIDocumentPickerDelegate, UIDocumentPickerViewController, UIInterfaceOrientationMask, UIViewController, UIWindowSceneGeometryPreferencesIOS};
use objc2_uniform_type_identifiers::UTType;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::platform::ios::{ScreenEdge, ValidOrientations, WindowExtIOS};
use slint::{ComponentHandle, Timer, Weak};
use tracing::{error, info, warn};

use crate::AppWindow;
use crate::settings::Orientation;

const ANY_ITEM: &str = "public.item";
const FOLDER_ITEM: &str = "public.folder";
const WINDOW_TRIES: u32 = 25;
const WINDOW_RETRY: Duration = Duration::from_millis(200);
const TRANSIENT_BELOW_SECONDS: f64 = 0.03;
const MILLIS_PER_SECOND: f64 = 1000.0;
const IMMEDIATELY: f64 = 0.0;

struct Ivars {
    request: i32,
    folder: bool,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "BccPickerDelegate"]
    #[ivars = Ivars]
    struct PickerDelegate;

    unsafe impl NSObjectProtocol for PickerDelegate {}

    unsafe impl UIDocumentPickerDelegate for PickerDelegate {
        #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
        fn did_pick(&self, _picker: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
            if self.ivars().folder {
                hold(urls);
            }

            let paths: Vec<String> = urls.iter().filter_map(|url| url.path().map(|path| path.to_string())).collect();

            deliver(self.ivars().request, (!paths.is_empty()).then_some(paths));
        }

        #[unsafe(method(documentPickerWasCancelled:))]
        fn cancelled(&self, _picker: &UIDocumentPickerViewController) {
            deliver(self.ivars().request, None);
        }
    }
);

impl PickerDelegate {
    fn new(mtm: MainThreadMarker, request: i32, folder: bool) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { request, folder });

        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    static DELEGATE: RefCell<Option<Retained<PickerDelegate>>> = const { RefCell::new(None) };
    static WINDOW: RefCell<Option<Weak<AppWindow>>> = const { RefCell::new(None) };
    static SCOPED: RefCell<Vec<Retained<NSURL>>> = const { RefCell::new(Vec::new()) };
}

fn hold(urls: &NSArray<NSURL>) {
    SCOPED.with(|held| {
        let mut held = held.borrow_mut();

        for url in held.drain(..) {
            unsafe { url.stopAccessingSecurityScopedResource() };
        }

        held.extend(urls.iter().filter(|url| unsafe { url.startAccessingSecurityScopedResource() }));
    });
}

fn deliver(request: i32, paths: Option<Vec<String>>) {
    DELEGATE.with(|held| *held.borrow_mut() = None);

    if let Err(err) = slint::invoke_from_event_loop(move || crate::picker::picked(request, paths)) {
        error!("The picked documents could not reach the app: {err}");
    }
}

fn presenter(ui: &AppWindow) -> Result<Retained<UIViewController>, String> {
    let handle = ui.window().window_handle();
    let raw = handle.window_handle().map_err(|err| format!("the window handle is unavailable: {err}"))?;
    let RawWindowHandle::UiKit(uikit) = raw.as_raw() else {
        return Err("the window is not a UIKit window".to_owned());
    };
    let controller: NonNull<UIViewController> = uikit.ui_view_controller.ok_or("the window has no view controller")?.cast();

    unsafe { Retained::retain(controller.as_ptr()) }.ok_or_else(|| "the view controller could not be retained".to_owned())
}

pub fn pick(ui: &AppWindow, multiple: bool, request: i32) -> Result<(), String> {
    open_picker(ui, request, ANY_ITEM, multiple, false)
}

pub fn pick_folder(ui: &AppWindow, request: i32) -> Result<(), String> {
    open_picker(ui, request, FOLDER_ITEM, false, true)
}

fn open_picker(ui: &AppWindow, request: i32, kind: &str, multiple: bool, folder: bool) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("the document picker must open on the main thread")?;
    let wanted = UTType::typeWithIdentifier(&NSString::from_str(kind)).ok_or_else(|| format!("the document type {kind} is unknown"))?;
    let types = NSArray::from_retained_slice(&[wanted]);
    let picker = UIDocumentPickerViewController::initForOpeningContentTypes_asCopy(mtm.alloc(), &types, !folder);
    let delegate = PickerDelegate::new(mtm, request, folder);

    picker.setAllowsMultipleSelection(multiple);
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    DELEGATE.with(|held| *held.borrow_mut() = Some(delegate));

    let presenter = presenter(ui)?;

    presenter.presentViewController_animated_completion(&picker, true, None);

    Ok(())
}

pub fn save(ui: &AppWindow, path: &Path, request: i32) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("the save dialog must open on the main thread")?;
    let absolute = std::path::absolute(path).map_err(|err| format!("{} has no absolute path: {err}", path.display()))?;
    let text = absolute.to_str().ok_or_else(|| format!("{} is not valid text", path.display()))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(text));
    let urls = NSArray::from_retained_slice(&[url]);
    let picker = UIDocumentPickerViewController::initForExportingURLs_asCopy(mtm.alloc(), &urls, true);
    let delegate = PickerDelegate::new(mtm, request, false);

    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    DELEGATE.with(|held| *held.borrow_mut() = Some(delegate));

    let presenter = presenter(ui)?;

    presenter.presentViewController_animated_completion(&picker, true, None);

    Ok(())
}

pub fn open_folder(path: &Path) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("a folder can only be opened from the main thread")?;
    let address = NSString::from_str(&format!("shareddocuments://{}", path.display()));
    let url = NSURL::URLWithString(&address).ok_or_else(|| format!("{} has no address", path.display()))?;

    unsafe { UIApplication::sharedApplication(mtm).openURL_options_completionHandler(&url, &NSDictionary::new(), None) };

    Ok(())
}

pub fn watch_window(window: &AppWindow) {
    WINDOW.with(|held| *held.borrow_mut() = Some(window.as_weak()));
}

static ACTIVE: AtomicBool = AtomicBool::new(true);

thread_local! {
    static OBSERVERS: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>> = const { RefCell::new(Vec::new()) };
}

pub fn app_active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

pub fn on_shown(handler: impl Fn(bool) + 'static) {
    let handler = Rc::new(handler);
    let center = NSNotificationCenter::defaultCenter();
    let main = NSOperationQueue::mainQueue();
    let changes = unsafe { [(UIApplicationWillResignActiveNotification, false), (UIApplicationDidBecomeActiveNotification, true)] };

    for (name, shown) in changes {
        let handler = Rc::clone(&handler);
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            ACTIVE.store(shown, Ordering::Release);
            info!(shown, "The app was shown or hidden");
            handler(shown);
        });
        let observer = unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, Some(&main), &block) };

        OBSERVERS.with(|held| held.borrow_mut().push(observer));
    }
}

pub fn set_fullscreen(fullscreen: bool) -> Result<(), String> {
    apply_fullscreen(fullscreen, WINDOW_TRIES)
}

fn apply_fullscreen(fullscreen: bool, tries: u32) -> Result<(), String> {
    let ui = WINDOW.with(|held| held.borrow().as_ref().and_then(Weak::upgrade)).ok_or("the window is gone")?;
    let edges = if fullscreen { ScreenEdge::ALL } else { ScreenEdge::NONE };
    let applied = ui.window().with_winit_window(|window| {
        window.set_prefers_status_bar_hidden(fullscreen);
        window.set_prefers_home_indicator_hidden(fullscreen);
        window.set_preferred_screen_edges_deferring_system_gestures(edges);
    });

    if applied.is_some() {
        return Ok(());
    }

    if tries == 0 {
        return Err("the window never appeared".to_owned());
    }

    Timer::single_shot(WINDOW_RETRY, move || {
        if let Err(err) = apply_fullscreen(fullscreen, tries - 1) {
            warn!("The system bars could not be changed: {err}");
        }
    });

    Ok(())
}

enum Haptic {
    Play { strength: f32, seconds: f64, sharpness: f32 },
    Cancel,
}

static HAPTICS: OnceLock<Option<Sender<Haptic>>> = OnceLock::new();

fn haptics() -> Option<&'static Sender<Haptic>> {
    HAPTICS
        .get_or_init(|| {
            let (sender, requests) = mpsc::channel();

            thread::Builder::new()
                .name("haptics".to_owned())
                .spawn(move || run_haptics(&requests))
                .inspect_err(|err| error!("The haptics thread could not be started: {err}"))
                .ok()
                .map(|_| sender)
        })
        .as_ref()
}

fn run_haptics(requests: &Receiver<Haptic>) {
    let mut engine = None;
    let mut playing: Option<Retained<ProtocolObject<dyn CHHapticPatternPlayer>>> = None;

    while let Ok(first) = requests.recv() {
        let latest = requests.try_iter().last().unwrap_or(first);

        autoreleasepool(|_| {
            if let Some(player) = playing.take() {
                let _ = unsafe { player.cancelAndReturnError() };
            }

            if let Haptic::Play { strength, seconds, sharpness } = latest {
                playing = play_haptic(&mut engine, strength, seconds, sharpness).inspect_err(|err| warn!("The phone could not vibrate: {err}")).ok();
            }
        });
    }
}

fn play_haptic(engine: &mut Option<Retained<CHHapticEngine>>, strength: f32, seconds: f64, sharpness: f32) -> Result<Retained<ProtocolObject<dyn CHHapticPatternPlayer>>, Retained<NSError>> {
    unsafe {
        let held = match engine.take() {
            Some(held) => held,
            None => {
                let made = CHHapticEngine::initAndReturnError(CHHapticEngine::alloc())?;

                made.setPlaysHapticsOnly(true);
                made.setAutoShutdownEnabled(true);
                made
            }
        };

        held.startAndReturnError()?;

        let parameters = NSArray::from_retained_slice(&[
            CHHapticEventParameter::initWithParameterID_value(CHHapticEventParameter::alloc(), CHHapticEventParameterIDHapticIntensity, strength),
            CHHapticEventParameter::initWithParameterID_value(CHHapticEventParameter::alloc(), CHHapticEventParameterIDHapticSharpness, sharpness),
        ]);
        let event = if seconds < TRANSIENT_BELOW_SECONDS {
            CHHapticEvent::initWithEventType_parameters_relativeTime(CHHapticEvent::alloc(), CHHapticEventTypeHapticTransient, &parameters, IMMEDIATELY)
        } else {
            CHHapticEvent::initWithEventType_parameters_relativeTime_duration(CHHapticEvent::alloc(), CHHapticEventTypeHapticContinuous, &parameters, IMMEDIATELY, seconds)
        };
        let pattern = CHHapticPattern::initWithEvents_parameters_error(CHHapticPattern::alloc(), &NSArray::from_retained_slice(&[event]), &NSArray::new())?;
        let player = held.createPlayerWithPattern_error(&pattern)?;

        player.startAtTime_error(IMMEDIATELY)?;
        *engine = Some(held);

        Ok(player)
    }
}

pub fn vibrate(strength: f32, millis: i32, sharpness: f32) {
    if !has_vibrator() {
        return;
    }

    let request = Haptic::Play { strength, seconds: f64::from(millis) / MILLIS_PER_SECOND, sharpness };

    if haptics().is_some_and(|sender| sender.send(request).is_err()) {
        warn!("The haptics thread is gone");
    }
}

pub fn cancel_vibration() {
    if haptics().is_some_and(|sender| sender.send(Haptic::Cancel).is_err()) {
        warn!("The haptics thread is gone");
    }
}

pub fn has_vibrator() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();

    *SUPPORTED.get_or_init(|| unsafe { CHHapticEngine::capabilitiesForHardware().supportsHaptics() })
}

pub fn set_orientation(orientation: Orientation) -> Result<(), String> {
    apply_orientation(orientation, WINDOW_TRIES)
}

fn apply_orientation(orientation: Orientation, tries: u32) -> Result<(), String> {
    let ui = WINDOW.with(|held| held.borrow().as_ref().and_then(Weak::upgrade)).ok_or("the window is gone")?;
    let (valid, mask) = match orientation {
        Orientation::Automatic => (ValidOrientations::LandscapeAndPortrait, UIInterfaceOrientationMask::AllButUpsideDown),
        Orientation::Portrait => (ValidOrientations::Portrait, UIInterfaceOrientationMask::Portrait),
        Orientation::Landscape => (ValidOrientations::Landscape, UIInterfaceOrientationMask::Landscape),
    };

    if ui.window().with_winit_window(|window| window.set_valid_orientations(valid)).is_some() {
        return rotate_scene(&ui, mask);
    }

    if tries == 0 {
        return Err("the window never appeared".to_owned());
    }

    Timer::single_shot(WINDOW_RETRY, move || {
        if let Err(err) = apply_orientation(orientation, tries - 1) {
            warn!("The screen orientation could not be changed: {err}");
        }
    });

    Ok(())
}

fn rotate_scene(ui: &AppWindow, mask: UIInterfaceOrientationMask) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("the orientation must change on the main thread")?;
    let controller = presenter(ui)?;
    let scene = controller.view().and_then(|view| view.window()).and_then(|window| window.windowScene()).ok_or("the window has no scene")?;

    if !scene.respondsToSelector(sel!(requestGeometryUpdateWithPreferences:errorHandler:)) {
        return Ok(());
    }

    controller.setNeedsUpdateOfSupportedInterfaceOrientations();

    let preferences = UIWindowSceneGeometryPreferencesIOS::initWithInterfaceOrientations(mtm.alloc(), mask);

    scene.requestGeometryUpdateWithPreferences_errorHandler(&preferences, None);

    Ok(())
}
