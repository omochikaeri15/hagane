#![cfg(target_os = "ios")]

use std::cell::RefCell;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSString, NSURL};
use objc2_ui_kit::{UIDocumentPickerDelegate, UIDocumentPickerViewController, UIViewController};
use objc2_uniform_type_identifiers::UTType;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use tracing::error;

use crate::AppWindow;

const ANY_ITEM: &str = "public.item";

struct Ivars {
    request: i32,
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
    fn new(mtm: MainThreadMarker, request: i32) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { request });

        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    static DELEGATE: RefCell<Option<Retained<PickerDelegate>>> = const { RefCell::new(None) };
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
    let mtm = MainThreadMarker::new().ok_or("the document picker must open on the main thread")?;
    let any = UTType::typeWithIdentifier(&NSString::from_str(ANY_ITEM)).ok_or("the generic document type is unknown")?;
    let types = NSArray::from_retained_slice(&[any]);
    let picker = unsafe { UIDocumentPickerViewController::initForOpeningContentTypes_asCopy(mtm.alloc(), &types, true) };
    let delegate = PickerDelegate::new(mtm, request);

    picker.setAllowsMultipleSelection(multiple);
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    DELEGATE.with(|held| *held.borrow_mut() = Some(delegate));

    let presenter = presenter(ui)?;

    presenter.presentViewController_animated_completion(&picker, true, None);

    Ok(())
}
