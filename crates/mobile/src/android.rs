#![cfg(target_os = "android")]

use std::cell::RefCell;
use std::env;
use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::OnceLock;
use std::time::Duration;

use jni::objects::{GlobalRef, JClass, JIntArray, JObject, JObjectArray, JString, JValue};
use jni::sys::{jboolean, jint};
use jni::{JNIEnv, JavaVM, NativeMethod};
use slint::android::AndroidApp;
use slint::{ComponentHandle, PhysicalSize, Timer, TimerMode};
use tracing::{debug, error};

use foundry::domains::import::device::Installed;
use foundry::domains::mining::Build;

use crate::paths::Home;
use crate::settings::Orientation;
use crate::AppWindow;

static APP: OnceLock<AndroidApp> = OnceLock::new();
static GLUE: OnceLock<GlobalRef> = OnceLock::new();

const GLUE_DEX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/classes.dex"));
const GLUE_CLASS: &str = "app.omochikaeri.hagane.Glue";

type ShownHandler = Box<dyn Fn(bool)>;

thread_local! {
    static WATCH: RefCell<Option<Timer>> = const { RefCell::new(None) };
    static SHOWN: RefCell<Option<ShownHandler>> = const { RefCell::new(None) };
}

const WATCH_INTERVAL: Duration = Duration::from_millis(250);

pub fn watch_window(window: &AppWindow) {
    let weak = window.as_weak();
    let timer = Timer::default();

    timer.start(TimerMode::Repeated, WATCH_INTERVAL, move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(native) = APP.get().and_then(AndroidApp::native_window) else {
            return;
        };
        let (width, height) = (native.width().max(0) as u32, native.height().max(0) as u32);
        let shown = ui.window().size();

        if width > 0 && height > 0 && (shown.width != width || shown.height != height) {
            debug!(width, height, "The native window changed size without a resize event");
            ui.window().set_size(PhysicalSize::new(width, height));
        }
    });

    WATCH.with(|held| *held.borrow_mut() = Some(timer));
}

const MESA_EXTENSION_OVERRIDE: &str = "MESA_EXTENSION_OVERRIDE";
const HIDDEN_EXTENSIONS: [&str; 10] = [
    "GL_EXT_base_instance",
    "GL_EXT_blend_func_extended",
    "GL_EXT_clear_texture",
    "GL_EXT_discard_framebuffer",
    "GL_EXT_disjoint_timer_query",
    "GL_EXT_multisampled_render_to_texture",
    "GL_EXT_window_rectangles",
    "GL_KHR_debug",
    "GL_NV_texture_barrier",
    "GL_OES_mapbuffer",
];

fn hide_optional_gl_extensions() {
    let hidden = HIDDEN_EXTENSIONS.iter().map(|name| format!("-{name}")).collect::<Vec<_>>().join(" ");

    unsafe {
        env::set_var(MESA_EXTENSION_OVERRIDE, hidden);
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let data = app.external_data_path().or_else(|| app.internal_data_path()).unwrap_or_else(|| PathBuf::from("/data/local/tmp/hagane"));
    let private = app.internal_data_path().unwrap_or_else(|| data.join("private"));
    let home = Home { data, private: Some(private) };

    let _ = APP.set(app.clone());

    hide_optional_gl_extensions();

    if let Err(err) = slint::android::init(app) {
        error!("The Android backend could not start: {err}");
        process::exit(1);
    }

    if let Err(err) = crate::run(home) {
        error!("The app stopped: {err}");
    }

    process::exit(0);
}

const ORIENTATION_UNSPECIFIED: i32 = -1;
const ORIENTATION_LANDSCAPE: i32 = 0;
const ORIENTATION_PORTRAIT: i32 = 1;

pub fn set_orientation(orientation: Orientation) -> Result<(), String> {
    let app = APP.get().ok_or("the Android app handle is not set")?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|err| err.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let requested = match orientation {
        Orientation::Automatic => ORIENTATION_UNSPECIFIED,
        Orientation::Portrait => ORIENTATION_PORTRAIT,
        Orientation::Landscape => ORIENTATION_LANDSCAPE,
    };

    env.call_method(&activity, "setRequestedOrientation", "(I)V", &[JValue::Int(requested)]).map_err(|err| err.to_string())?;

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        return Err("the system refused the orientation".to_owned());
    }

    Ok(())
}

pub fn open_url(url: &str) -> Result<(), String> {
    let app = APP.get().ok_or("the Android app handle is not set")?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|err| err.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let uri_text = env.new_string(url).map_err(|err| err.to_string())?;
    let uri = env
        .call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&uri_text)])
        .and_then(|value| value.l())
        .map_err(|err| err.to_string())?;
    let action = env.new_string("android.intent.action.VIEW").map_err(|err| err.to_string())?;
    let intent = env
        .new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&action), JValue::Object(&uri)])
        .map_err(|err| err.to_string())?;

    env.call_method(&activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)]).map_err(|err| err.to_string())?;

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        return Err("the system refused to open the link".to_owned());
    }

    Ok(())
}

pub fn installed(package: &str) -> Result<Option<Installed>, String> {
    let app = APP.get().ok_or("the Android app handle is not set")?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|err| err.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let manager = env.call_method(&activity, "getPackageManager", "()Landroid/content/pm/PackageManager;", &[]).and_then(|value| value.l()).map_err(|err| err.to_string())?;
    let name = env.new_string(package).map_err(|err| err.to_string())?;
    let info = env.call_method(&manager, "getApplicationInfo", "(Ljava/lang/String;I)Landroid/content/pm/ApplicationInfo;", &[JValue::Object(&name), JValue::Int(0)]);

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();

        return Ok(None);
    }

    let info = info.and_then(|value| value.l()).map_err(|err| err.to_string())?;
    let base = text_field(&mut env, &info, "sourceDir")?.ok_or("the installed app has no base APK")?;
    let splits = env.get_field(&info, "splitSourceDirs", "[Ljava/lang/String;").and_then(|value| value.l()).map_err(|err| err.to_string())?;
    let mut split_paths = Vec::new();

    if !splits.is_null() {
        let array = JObjectArray::from(splits);
        let count = env.get_array_length(&array).map_err(|err| err.to_string())?;

        for index in 0..count {
            let element = env.get_object_array_element(&array, index).map_err(|err| err.to_string())?;

            if !element.is_null() {
                let text: String = env.get_string(&JString::from(element)).map_err(|err| err.to_string())?.into();

                split_paths.push(PathBuf::from(text));
            }
        }
    }

    let package_info = env.call_method(&manager, "getPackageInfo", "(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;", &[JValue::Object(&name), JValue::Int(0)]);
    let build = if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();

        None
    } else {
        let package_info = package_info.and_then(|value| value.l()).map_err(|err| err.to_string())?;
        let version = text_field(&mut env, &package_info, "versionName")?.filter(|version| !version.is_empty());
        let code = env.get_field(&package_info, "versionCode", "I").and_then(|value| value.i()).map_err(|err| err.to_string())?;

        version.map(|version| Build { code: u32::try_from(code).unwrap_or_default(), name: version, label: package.to_owned() })
    };

    Ok(Some(Installed { package: package.to_owned(), base: PathBuf::from(base), splits: split_paths, build }))
}

fn text_field(env: &mut JNIEnv, object: &JObject, field: &str) -> Result<Option<String>, String> {
    let value = env.get_field(object, field, "Ljava/lang/String;").and_then(|value| value.l()).map_err(|err| err.to_string())?;

    if value.is_null() {
        return Ok(None);
    }

    let text: String = env.get_string(&JString::from(value)).map_err(|err| err.to_string())?.into();

    Ok(Some(text))
}

const MEDIA_STORE_API: i32 = 29;
const FLAG_ACTIVITY_NEW_TASK: i32 = 0x1000_0000;
const FLAG_GRANT_READ_URI_PERMISSION: i32 = 1;
const DOWNLOADS_ROOT: &str = "Download/Hagane";
const APK_MIME: &str = "application/vnd.android.package-archive";

fn jni<'local, T>(env: &mut JNIEnv<'local>, what: &str, call: impl FnOnce(&mut JNIEnv<'local>) -> jni::errors::Result<T>) -> Result<T, String> {
    call(env).map_err(|err| {
        let thrown = env.exception_occurred().ok().filter(|thrown| !thrown.is_null());
        let _ = env.exception_clear();
        let detail = thrown.and_then(|thrown| {
            let text = |env: &mut JNIEnv<'local>, object: &JObject| {
                let message = env.call_method(object, "toString", "()Ljava/lang/String;", &[]).and_then(|value| value.l()).ok()?;

                env.get_string(&JString::from(message)).ok().map(String::from)
            };
            let message = text(env, &thrown)?;
            let frame = env
                .call_method(&thrown, "getStackTrace", "()[Ljava/lang/StackTraceElement;", &[])
                .and_then(|value| value.l())
                .ok()
                .and_then(|array| env.get_object_array_element(&JObjectArray::from(array), 0).ok())
                .filter(|element| !element.is_null())
                .and_then(|element| text(env, &element));

            Some(frame.map_or(message.clone(), |frame| format!("{message} at {frame}")))
        });

        detail.map_or_else(|| format!("{what}: {err}"), |detail| format!("{what}: {detail}"))
    })
}

fn sdk_int(env: &mut JNIEnv) -> Result<i32, String> {
    jni(env, "the Android version could not be read", |env| env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I").and_then(|value| value.i()))
}

fn put_text(env: &mut JNIEnv, values: &JObject, key: &str, value: &str) -> Result<(), String> {
    let key = jni(env, "a column name could not be created", |env| env.new_string(key))?;
    let value = jni(env, "a column value could not be created", |env| env.new_string(value))?;

    jni(env, "a column could not be set", |env| env.call_method(values, "put", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&key), JValue::Object(&value)])).map(|_| ())
}

fn put_int(env: &mut JNIEnv, values: &JObject, key: &str, value: i32) -> Result<(), String> {
    let key = jni(env, "a column name could not be created", |env| env.new_string(key))?;
    let boxed = jni(env, "a number could not be boxed", |env| env.call_static_method("java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;", &[JValue::Int(value)]).and_then(|value| value.l()))?;

    jni(env, "a column could not be set", |env| env.call_method(values, "put", "(Ljava/lang/String;Ljava/lang/Integer;)V", &[JValue::Object(&key), JValue::Object(&boxed)])).map(|_| ())
}

pub fn publish(path: &Path, folder: &str, mime: &str) -> Result<String, String> {
    let app = APP.get().ok_or("the Android app handle is not set")?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|err| err.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };

    if sdk_int(&mut env)? < MEDIA_STORE_API {
        return Err("Android 10 or newer is needed to write into Downloads".to_owned());
    }

    let name = path.file_name().and_then(|name| name.to_str()).ok_or("the file has no name")?;
    let relative = if folder.is_empty() { DOWNLOADS_ROOT.to_owned() } else { format!("{DOWNLOADS_ROOT}/{}", folder.trim_matches('/')) };
    let values = jni(&mut env, "the file entry could not be described", |env| env.new_object("android/content/ContentValues", "()V", &[]))?;

    put_text(&mut env, &values, "_display_name", name)?;
    put_text(&mut env, &values, "mime_type", mime)?;
    put_text(&mut env, &values, "relative_path", &relative)?;
    put_int(&mut env, &values, "is_pending", 1)?;

    let collection = jni(&mut env, "the Downloads collection could not be reached", |env| {
        env.get_static_field("android/provider/MediaStore$Downloads", "EXTERNAL_CONTENT_URI", "Landroid/net/Uri;").and_then(|value| value.l())
    })?;
    let resolver = jni(&mut env, "the content resolver could not be reached", |env| env.call_method(&activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[]).and_then(|value| value.l()))?;
    let uri = jni(&mut env, "the system refused to create the file in Downloads", |env| {
        env.call_method(&resolver, "insert", "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;", &[JValue::Object(&collection), JValue::Object(&values)]).and_then(|value| value.l())
    })?;

    if uri.is_null() {
        return Err("the system refused to create the file in Downloads".to_owned());
    }

    let mode = jni(&mut env, "the open mode could not be created", |env| env.new_string("w"))?;
    let descriptor = jni(&mut env, "the Downloads file could not be opened", |env| {
        env.call_method(&resolver, "openFileDescriptor", "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;", &[JValue::Object(&uri), JValue::Object(&mode)]).and_then(|value| value.l())
    })?;
    let fd = jni(&mut env, "the Downloads file could not be detached", |env| env.call_method(&descriptor, "detachFd", "()I", &[]).and_then(|value| value.i()))?;
    let mut out = unsafe { File::from_raw_fd(fd) };
    let mut source = File::open(path).map_err(|err| format!("{} could not be read: {err}", path.display()))?;

    io::copy(&mut source, &mut out).map_err(|err| format!("{name} could not be written into Downloads: {err}"))?;
    drop(out);

    let done = jni(&mut env, "the file entry could not be described", |env| env.new_object("android/content/ContentValues", "()V", &[]))?;

    put_int(&mut env, &done, "is_pending", 0)?;
    jni(&mut env, "the Downloads file could not be finished", |env| {
        env.call_method(
            &resolver,
            "update",
            "(Landroid/net/Uri;Landroid/content/ContentValues;Ljava/lang/String;[Ljava/lang/String;)I",
            &[JValue::Object(&uri), JValue::Object(&done), JValue::Object(&JObject::null()), JValue::Object(&JObject::null())],
        )
    })?;

    let text = jni(&mut env, "the file address could not be read", |env| env.call_method(&uri, "toString", "()Ljava/lang/String;", &[]).and_then(|value| value.l()))?;
    let text = JString::from(text);
    let text: String = jni(&mut env, "the file address could not be read", |env| env.get_string(&text).map(String::from))?;

    Ok(text)
}

pub fn install(uri: &str) -> Result<(), String> {
    let app = APP.get().ok_or("the Android app handle is not set")?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|err| err.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let uri_text = jni(&mut env, "the package address could not be created", |env| env.new_string(uri))?;
    let parsed = jni(&mut env, "the package address could not be parsed", |env| {
        env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&uri_text)]).and_then(|value| value.l())
    })?;
    let action = jni(&mut env, "the intent action could not be created", |env| env.new_string("android.intent.action.VIEW"))?;
    let intent = jni(&mut env, "the install intent could not be created", |env| env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&action)]))?;
    let mime = jni(&mut env, "the package type could not be created", |env| env.new_string(APK_MIME))?;

    jni(&mut env, "the install intent could not be addressed", |env| {
        env.call_method(&intent, "setDataAndType", "(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&parsed), JValue::Object(&mime)])
    })?;
    jni(&mut env, "the install intent could not be flagged", |env| env.call_method(&intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(FLAG_ACTIVITY_NEW_TASK | FLAG_GRANT_READ_URI_PERMISSION)]))?;
    jni(&mut env, "the system refused to open the package installer", |env| env.call_method(&activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])).map(|_| ())
}

fn glue_class<'local>(env: &mut JNIEnv<'local>, activity: &JObject) -> Result<JClass<'local>, String> {
    if GLUE.get().is_none() {
        let buffer = jni(env, "the glue code could not be wrapped", |env| unsafe { env.new_direct_byte_buffer(GLUE_DEX.as_ptr().cast_mut(), GLUE_DEX.len()) })?;
        let parent = jni(env, "the app class loader could not be reached", |env| env.call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[]).and_then(|value| value.l()))?;
        let loader = jni(env, "the glue code could not be loaded", |env| {
            env.new_object("dalvik/system/InMemoryDexClassLoader", "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V", &[JValue::Object(&buffer), JValue::Object(&parent)])
        })?;
        let name = jni(env, "the glue class name could not be created", |env| env.new_string(GLUE_CLASS))?;
        let class = jni(env, "the glue class could not be found", |env| env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)]).and_then(|value| value.l()))?;
        let class = JClass::from(class);
        let methods = [
            NativeMethod { name: "picked".into(), sig: "(I[Ljava/lang/String;)V".into(), fn_ptr: picked as *const () as *mut c_void },
            NativeMethod { name: "shown".into(), sig: "(Z)V".into(), fn_ptr: shown as *const () as *mut c_void },
        ];

        jni(env, "the picker callback could not be bound", |env| env.register_native_methods(&class, &methods))?;

        let global = jni(env, "the glue class could not be kept", |env| env.new_global_ref(&class))?;
        let _ = GLUE.set(global);
    }

    let held = GLUE.get().ok_or("the glue class is not loaded")?;

    jni(env, "the glue class could not be referenced", |env| env.new_local_ref(held.as_obj())).map(JClass::from)
}

extern "system" fn shown(_env: JNIEnv, _class: JClass, visible: jboolean) {
    let visible = visible != 0;
    let delivered = slint::invoke_from_event_loop(move || {
        SHOWN.with(|handler| {
            if let Some(handler) = handler.borrow().as_ref() {
                handler(visible);
            }
        });
    });

    if let Err(err) = delivered {
        error!("The app could not be told it was shown or hidden: {err}");
    }
}

pub fn on_shown(handler: impl Fn(bool) + 'static) {
    SHOWN.with(|held| *held.borrow_mut() = Some(Box::new(handler)));
}

extern "system" fn picked(mut env: JNIEnv, _class: JClass, request: jint, uris: JObjectArray) {
    let mut found = Vec::new();

    if !uris.is_null() {
        let count = env.get_array_length(&uris).unwrap_or(0);

        for index in 0..count {
            if let Ok(item) = env.get_object_array_element(&uris, index) {
                let item = JString::from(item);

                if let Ok(text) = env.get_string(&item) {
                    found.push(String::from(text));
                }
            }
        }
    }

    let uris = (!found.is_empty()).then_some(found);

    if let Err(err) = slint::invoke_from_event_loop(move || crate::picker::picked(request, uris)) {
        error!("The picked documents could not reach the app: {err}");
    }
}

fn with_glue<T>(what: &str, call: impl FnOnce(&mut JNIEnv, &JObject, &JClass) -> jni::errors::Result<T>) -> Result<T, String> {
    let app = APP.get().ok_or("the Android app handle is not set")?;
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|err| err.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|err| err.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let class = glue_class(&mut env, &activity)?;

    jni(&mut env, what, |env| call(env, &activity, &class))
}

pub fn pick(multiple: bool, request: i32) -> Result<(), String> {
    with_glue("the system file picker could not be opened", |env, activity, class| {
        env.call_static_method(class, "pick", "(Landroid/app/Activity;ZI)V", &[JValue::Object(activity), JValue::Bool(u8::from(multiple)), JValue::Int(request)]).map(|_| ())
    })
}

pub fn save(name: &str, mime: &str, request: i32) -> Result<(), String> {
    with_glue("the save dialog could not be opened", |env, activity, class| {
        let name = env.new_string(name)?;
        let mime = env.new_string(mime)?;

        env.call_static_method(class, "save", "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;I)V", &[JValue::Object(activity), JValue::Object(&name), JValue::Object(&mime), JValue::Int(request)]).map(|_| ())
    })
}

pub fn set_bars(fullscreen: bool) -> Result<(), String> {
    with_glue("the system bars could not be changed", |env, activity, class| {
        env.call_static_method(class, "bars", "(Landroid/app/Activity;Z)V", &[JValue::Object(activity), JValue::Bool(u8::from(fullscreen))]).map(drop)
    })
}

pub fn screen_facts() -> Result<[i32; 11], String> {
    with_glue("the screen could not be read", |env, activity, class| {
        let facts = env.call_static_method(class, "screenFacts", "(Landroid/app/Activity;)[I", &[JValue::Object(activity)]).and_then(|value| value.l())?;
        let mut read = [0; 11];

        env.get_int_array_region(JIntArray::from(facts), 0, &mut read)?;

        Ok(read)
    })
}

pub fn vibrate(strength: f32, millis: i32) -> Result<(), String> {
    with_glue("the phone could not vibrate", |env, activity, class| {
        env.call_static_method(class, "vibrate", "(Landroid/app/Activity;FI)V", &[JValue::Object(activity), JValue::Float(strength), JValue::Int(millis)]).map(drop)
    })
}

pub fn cancel_vibration() -> Result<(), String> {
    with_glue("the vibration could not be cancelled", |env, activity, class| {
        env.call_static_method(class, "cancelVibration", "(Landroid/app/Activity;)V", &[JValue::Object(activity)]).map(drop)
    })
}

pub fn has_vibrator() -> Result<bool, String> {
    with_glue("the vibrator could not be queried", |env, activity, class| {
        env.call_static_method(class, "hasVibrator", "(Landroid/app/Activity;)Z", &[JValue::Object(activity)]).and_then(|value| value.z())
    })
}

pub fn has_mouse() -> Result<bool, String> {
    with_glue("the input devices could not be read", |env, _activity, class| {
        env.call_static_method(class, "hasMouse", "()Z", &[]).and_then(|value| value.z())
    })
}

pub fn copy_document(uri: &str, folder: &Path) -> Result<PathBuf, String> {
    let path = with_glue("the document could not be copied", |env, activity, class| {
        let uri = env.new_string(uri)?;
        let folder = env.new_string(folder.to_string_lossy())?;
        let path = env
            .call_static_method(class, "copy", "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(activity), JValue::Object(&uri), JValue::Object(&folder)])
            .and_then(|value| value.l())?;
        let path = JString::from(path);

        env.get_string(&path).map(String::from)
    })?;

    Ok(PathBuf::from(path))
}

pub fn write_document(uri: &str, path: &Path) -> Result<(), String> {
    with_glue("the document could not be written", |env, activity, class| {
        let uri = env.new_string(uri)?;
        let path = env.new_string(path.to_string_lossy())?;

        env.call_static_method(class, "write", "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(activity), JValue::Object(&uri), JValue::Object(&path)]).map(|_| ())
    })
}
