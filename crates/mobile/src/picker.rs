use std::borrow::Cow;
#[cfg(any(target_os = "android", target_os = "ios"))]
use std::cell::Cell;
use std::cell::RefCell;
#[cfg(any(target_os = "android", target_os = "ios"))]
use std::collections::HashMap;
use std::fs;
#[cfg(target_os = "android")]
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "android")]
use std::sync::atomic::Ordering;
#[cfg(any(target_os = "android", target_os = "ios"))]
use std::thread;

use slint::ComponentHandle;
#[cfg(any(target_os = "android", target_os = "ios"))]
use slint::Weak;
#[cfg(any(target_os = "android", target_os = "ios"))]
use slint::platform::WindowEvent;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use slint::{ModelRc, SharedString, VecModel};
#[cfg(any(target_os = "android", target_os = "ios"))]
use tracing::error;
use tracing::warn;

use foundry::common::dirs;
use foundry::common::job::{JobEvent, ProgressCounter};

use crate::pages::Context;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::paths;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::platform;
use crate::{AppWindow, Picker};

pub type Then = Box<dyn FnOnce(&AppWindow, &Rc<Context>, PathBuf)>;
pub type Landed = Box<dyn FnOnce(&AppWindow, &Rc<Context>, Source)>;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub type Saved = Box<dyn FnOnce(&AppWindow, &Rc<Context>, bool)>;

#[derive(Default)]
pub struct State {
    files: Vec<PathBuf>,
    then: Option<Then>,
}

pub type Shared = Rc<RefCell<State>>;

#[derive(Clone)]
pub enum Source {
    Path(PathBuf),
    #[cfg(target_os = "android")]
    Tree(Tree),
}

#[cfg(target_os = "android")]
#[derive(Clone)]
pub struct Tree {
    uri: String,
    name: String,
    into: PathBuf,
}

pub struct Staged<'a> {
    path: Cow<'a, Path>,
    #[cfg(target_os = "android")]
    owned: Option<&'a Path>,
}

#[cfg(any(target_os = "android", target_os = "ios"))]
enum Task {
    Open { then: Then, exts: Vec<String>, many: bool },
    Folder { then: Landed },
    Save { path: PathBuf, then: Saved },
}

#[cfg(any(target_os = "android", target_os = "ios"))]
struct Pending {
    task: Task,
    ctx: Rc<Context>,
    weak: Weak<AppWindow>,
}

#[cfg(any(target_os = "android", target_os = "ios"))]
thread_local! {
    static PENDING: RefCell<HashMap<i32, Pending>> = RefCell::new(HashMap::new());
    static NEXT: Cell<i32> = const { Cell::new(1) };
}

const PICKED: &str = "picked";
#[cfg(any(target_os = "android", target_os = "ios"))]
const UNREADABLE: &str = "The picked files could not be read";
#[cfg(any(target_os = "android", target_os = "ios"))]
const UNREADABLE_FOLDER: &str = "The picked folder could not be read";
#[cfg(target_os = "android")]
const ABORTED: &str = "the copy from the picked folder was aborted";
#[cfg(target_os = "android")]
const WALK_NOTE: usize = 5000;
#[cfg(target_os = "android")]
const COPY_NOTES: usize = 20;
#[cfg(any(target_os = "android", target_os = "ios"))]
const UNAVAILABLE: &str = "The file picker could not be opened";
#[cfg(any(target_os = "android", target_os = "ios"))]
const UNWRITABLE: &str = "The file could not be written where you chose";

fn picked_dir() -> Option<PathBuf> {
    dirs::cache().map(|cache| cache.join(PICKED))
}

pub fn sweep() {
    if let Some(dir) = picked_dir()
        && dir.exists()
        && let Err(err) = fs::remove_dir_all(&dir)
    {
        warn!(path = %dir.display(), "The picked documents could not be cleared: {err}");
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn matching(exts: &[&str]) -> Vec<PathBuf> {
    let directory = paths::import_dir();
    let Ok(entries) = fs::read_dir(&directory).inspect_err(|err| warn!(path = %directory.display(), "The import folder could not be read: {err}")) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| exts.iter().any(|wanted| wanted.eq_ignore_ascii_case(ext))))
        .collect();

    files.sort();
    files
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn folders() -> Vec<PathBuf> {
    let directory = paths::import_dir();
    let Ok(entries) = fs::read_dir(&directory).inspect_err(|err| warn!(path = %directory.display(), "The import folder could not be read: {err}")) else {
        return vec![directory];
    };
    let mut found: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();

    found.sort();
    found.insert(0, directory);
    found
}

pub fn open_many(ui: &AppWindow, context: &Rc<Context>, title: &str, exts: &[&str], then: Then) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = title;
        request(ui, context, Task::Open { then, exts: exts.iter().map(|ext| ext.to_ascii_lowercase()).collect(), many: true });
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = exts;
        show(ui, context, title, folders(), then);
    }
}

pub fn open(ui: &AppWindow, context: &Rc<Context>, title: &str, exts: &[&str], then: Then) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = title;
        request(ui, context, Task::Open { then, exts: exts.iter().map(|ext| ext.to_ascii_lowercase()).collect(), many: false });
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    show(ui, context, title, matching(exts), then);
}

pub fn open_folder(ui: &AppWindow, context: &Rc<Context>, title: &str, then: Landed) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = title;
        request(ui, context, Task::Folder { then });
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    show(ui, context, title, folders(), Box::new(move |ui, ctx, path| then(ui, ctx, Source::Path(path))));
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn save(ui: &AppWindow, context: &Rc<Context>, path: PathBuf, then: Saved) {
    request(ui, context, Task::Save { path, then });
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn request(ui: &AppWindow, context: &Rc<Context>, task: Task) {
    let id = NEXT.with(|next| {
        let id = next.get();

        next.set(id.wrapping_add(1).max(1));
        id
    });
    let opened = match &task {
        Task::Open { many, .. } => platform::pick(ui, *many, id),
        Task::Folder { .. } => platform::pick_folder(ui, id),
        Task::Save { path, .. } => platform::save(ui, path, crate::delivery::mime(path), id),
    };

    PENDING.with(|held| held.borrow_mut().insert(id, Pending { task, ctx: Rc::clone(context), weak: ui.as_weak() }));

    if let Err(err) = opened {
        error!("The system dialog could not be opened: {err}");

        if let Some(pending) = PENDING.with(|held| held.borrow_mut().remove(&id)) {
            match pending.task {
                Task::Open { .. } | Task::Folder { .. } => context.toast(ui, UNAVAILABLE, true),
                Task::Save { then, .. } => then(ui, context, false),
            }
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn picked(request: i32, uris: Option<Vec<String>>) {
    let kind = PENDING.with(|held| held.borrow().get(&request).map(|pending| (matches!(pending.task, Task::Save { .. }), matches!(pending.task, Task::Folder { .. }))));
    let Some((saving, folder)) = kind else {
        return;
    };

    PENDING.with(|held| {
        if let Some(ui) = held.borrow().get(&request).and_then(|pending| pending.weak.upgrade()) {
            ui.window().dispatch_event(WindowEvent::PointerExited);
        }
    });

    let Some(uris) = uris else {
        let Some(pending) = PENDING.with(|held| held.borrow_mut().remove(&request)) else {
            return;
        };

        if let (Task::Save { then, .. }, Some(ui)) = (pending.task, pending.weak.upgrade()) {
            then(&ui, &pending.ctx, false);
        }

        return;
    };

    if saving {
        let Some(uri) = uris.into_iter().next() else {
            return;
        };
        let source = PENDING.with(|held| held.borrow().get(&request).and_then(|pending| if let Task::Save { path, .. } = &pending.task { Some(path.clone()) } else { None }));
        let Some(source) = source else {
            return;
        };

        thread::spawn(move || {
            let result = platform::write_document(&uri, &source);

            if let Err(err) = slint::invoke_from_event_loop(move || finish_save(request, result)) {
                error!("The saved document could not reach the app: {err}");
            }
        });

        return;
    }

    if folder {
        let Some(uri) = uris.into_iter().next() else {
            return;
        };

        thread::spawn(move || {
            let result = landed(uri, request);

            if let Err(err) = slint::invoke_from_event_loop(move || finish_folder(request, result)) {
                error!("The picked folder could not reach the app: {err}");
            }
        });

        return;
    }

    let Some(folder) = picked_dir().map(|dir| dir.join(request.to_string())) else {
        finish_open(request, Err("no cache folder is available for picked documents".to_owned()));
        return;
    };

    thread::spawn(move || {
        let result = copy_all(&uris, &folder);

        if let Err(err) = slint::invoke_from_event_loop(move || finish_open(request, result)) {
            error!("The copied documents could not reach the app: {err}");
        }
    });
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn copy_all(uris: &[String], folder: &Path) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(folder).map_err(|err| format!("{} could not be created: {err}", folder.display()))?;
    uris.iter().map(|uri| platform::copy_document(uri, folder)).collect()
}

#[cfg(target_os = "android")]
fn landed(uri: String, request: i32) -> Result<Source, String> {
    let into = picked_dir().ok_or("no cache folder is available for picked documents")?.join(request.to_string());
    let name = platform::tree_name(&uri)?;

    Ok(Source::Tree(Tree { uri, name, into }))
}

#[cfg(target_os = "ios")]
fn landed(uri: String, _request: i32) -> Result<Source, String> {
    Ok(Source::Path(PathBuf::from(uri)))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn finish_folder(request: i32, result: Result<Source, String>) {
    let Some(pending) = PENDING.with(|held| held.borrow_mut().remove(&request)) else {
        return;
    };
    let Some(ui) = pending.weak.upgrade() else {
        return;
    };
    let Task::Folder { then } = pending.task else {
        return;
    };

    match result {
        Ok(source) => then(&ui, &pending.ctx, source),
        Err(err) => {
            error!("The picked folder could not be read: {err}");
            pending.ctx.toast(&ui, &format!("{UNREADABLE_FOLDER}: {err}"), true);
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn finish_save(request: i32, result: Result<(), String>) {
    let Some(pending) = PENDING.with(|held| held.borrow_mut().remove(&request)) else {
        return;
    };
    let Some(ui) = pending.weak.upgrade() else {
        return;
    };
    let Task::Save { then, .. } = pending.task else {
        return;
    };

    match result {
        Ok(()) => then(&ui, &pending.ctx, true),
        Err(err) => {
            error!("The export could not be written to the chosen document: {err}");
            pending.ctx.toast(&ui, UNWRITABLE, true);
            then(&ui, &pending.ctx, false);
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn finish_open(request: i32, result: Result<Vec<PathBuf>, String>) {
    let Some(pending) = PENDING.with(|held| held.borrow_mut().remove(&request)) else {
        return;
    };
    let Some(ui) = pending.weak.upgrade() else {
        return;
    };
    let Task::Open { then, exts, many } = pending.task else {
        return;
    };
    let paths = match result {
        Ok(paths) => paths,
        Err(err) => {
            error!("The picked documents could not be copied: {err}");
            pending.ctx.toast(&ui, &format!("{UNREADABLE}: {err}"), true);
            return;
        }
    };
    let (accepted, rejected): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|path| fits(path, &exts));

    for path in rejected {
        if let Err(err) = fs::remove_file(&path) {
            warn!(path = %path.display(), "A rejected document could not be removed: {err}");
        }
    }

    if accepted.is_empty() {
        pending.ctx.toast(&ui, &wanted(&exts), true);
        return;
    }

    let handed = if many { accepted.first().and_then(|path| path.parent()).map(Path::to_path_buf) } else { accepted.into_iter().next() };

    if let Some(path) = handed {
        then(&ui, &pending.ctx, path);
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn fits(path: &Path, exts: &[String]) -> bool {
    exts.is_empty() || path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| exts.iter().any(|wanted| wanted.eq_ignore_ascii_case(ext)))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn wanted(exts: &[String]) -> String {
    let listed: Vec<String> = exts.iter().map(|ext| format!(".{ext}")).collect();

    format!("Pick a {} file", listed.join(", "))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn show(ui: &AppWindow, context: &Rc<Context>, title: &str, files: Vec<PathBuf>, then: Then) {
    let picker = ui.global::<Picker>();
    let names: Vec<SharedString> = files.iter().map(|path| path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned()).into()).collect();

    picker.set_title(title.into());
    picker.set_hint(format!("Place files in {} and reopen this list", paths::import_dir().display()).into());
    picker.set_files(ModelRc::new(VecModel::from(names)));

    {
        let mut state = context.picker.borrow_mut();

        state.files = files;
        state.then = Some(then);
    }

    picker.set_open(true);
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) {
    let picker = window.global::<Picker>();

    {
        let (ctx, weak) = (Rc::clone(context), window.as_weak());

        picker.on_picked(move |index| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let taken = {
                let mut state = ctx.picker.borrow_mut();
                let path = state.files.get(index.max(0) as usize).cloned();

                path.zip(state.then.take())
            };

            if let Some((path, then)) = taken {
                then(&ui, &ctx, path);
            }
        });
    }

    {
        let ctx = Rc::clone(context);

        picker.on_canceled(move || {
            ctx.picker.borrow_mut().then = None;
        });
    }
}

pub fn label(path: &Path) -> String {
    const MAX: usize = 22;
    const TAIL: usize = 6;
    let name = path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let chars: Vec<char> = name.chars().collect();

    if chars.len() <= MAX {
        return name;
    }

    let head: String = chars[..MAX - TAIL - 3].iter().collect();
    let tail: String = chars[chars.len() - TAIL..].iter().collect();

    format!("{head}\u{2026}{tail}")
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Self::Path(path) => label(path),
            #[cfg(target_os = "android")]
            Self::Tree(tree) => label(Path::new(&tree.name)),
        }
    }

    pub fn stage(&self, emit: &(dyn Fn(JobEvent) + Sync), abort: &AtomicBool, progress: &ProgressCounter) -> Result<Staged<'_>, String> {
        match self {
            Self::Path(path) => {
                let _ = (emit, abort, progress);

                Ok(Staged {
                    path: Cow::Borrowed(path),
                    #[cfg(target_os = "android")]
                    owned: None,
                })
            }
            #[cfg(target_os = "android")]
            Self::Tree(tree) => tree.stage(emit, abort, progress),
        }
    }
}

impl Staged<'_> {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(target_os = "android")]
impl Drop for Staged<'_> {
    fn drop(&mut self) {
        if let Some(dir) = self.owned
            && let Err(err) = fs::remove_dir_all(dir)
            && err.kind() != ErrorKind::NotFound
        {
            warn!(path = %dir.display(), "The copy of the picked folder could not be removed: {err}");
        }
    }
}

#[cfg(target_os = "android")]
impl Tree {
    fn stage(&self, emit: &(dyn Fn(JobEvent) + Sync), abort: &AtomicBool, progress: &ProgressCounter) -> Result<Staged<'_>, String> {
        let staged = Staged { path: Cow::Owned(self.into.join(&self.name)), owned: Some(&self.into) };

        if let Err(err) = fs::remove_dir_all(&self.into)
            && err.kind() != ErrorKind::NotFound
        {
            return Err(format!("{} could not be cleared: {err}", self.into.display()));
        }

        emit(JobEvent::Log(format!("Reading the folder {}...", self.name)));

        let mut noted = 0;
        let files = platform::walk_tree(&self.uri, abort, &mut |count| {
            if count >= noted + WALK_NOTE {
                noted = count;
                emit(JobEvent::Log(format!("Found {count} files so far...")));
            }
        })?;

        if abort.load(Ordering::Relaxed) {
            return Err(ABORTED.to_owned());
        }

        let total = files.len();
        let step = (total / COPY_NOTES).max(1);

        emit(JobEvent::Log(format!("Copying {total} files from {}...", self.name)));
        emit(JobEvent::Progress { current: 0, total });
        progress.reset(total);

        platform::fetch_tree(&self.uri, &files, &staged.path, abort, &|| {
            let current = progress.advance();

            if current.is_multiple_of(step) || current == total {
                emit(JobEvent::Progress { current, total });
                emit(JobEvent::Log(format!("Copied {current} of {total} files...")));
            }
        })?;

        if abort.load(Ordering::Relaxed) {
            return Err(ABORTED.to_owned());
        }

        Ok(staged)
    }
}
