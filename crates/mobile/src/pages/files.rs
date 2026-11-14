use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{info, warn};

use foundry::Vfs;
use foundry::common::architecture;
use foundry::common::preview::{self, Preview, Stamp};
use foundry::domains::import::{self, PackFault};

use crate::images;
use crate::pages::Context;
use crate::picker;
use crate::{AppWindow, FileRow, Files, Page, Shell, bridge};

const NOTICE_MS: u64 = 3000;
const LOCKED_NOTICE: &str = "This mount is locked, unlock the game mount in Settings > Files to edit it";
const MANIFEST_BUILDING_NOTICE: &str = "The pack manifest is still being read";
const MANIFEST_MISSING_NOTICE: &str = "No pack manifest was found, import game files first";
const MANIFEST_MALFORMED_NOTICE: &str = "The pack manifest could not be read";
const OVERSIZED_LABEL: &str = "File Too Large to Preview";
const BINARY_LABEL: &str = "No Preview for Binary Files";
const EMPTY_LABEL: &str = "No Files Found on Mount";
const MISSING_LABEL: &str = "Selected Mount Missing from Memory";
const NO_MOUNTS_LABEL: &str = "No Mounts Available";
const HEADER_EMPTY: &str = "Please select a file";
const HEADER_SEPARATOR: &str = " :: ";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Tree,
    Flat,
    Pack,
}

impl Mode {
    fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Flat,
            2 => Self::Pack,
            _ => Self::Tree,
        }
    }

    fn index(self) -> i32 {
        match self {
            Self::Tree => 0,
            Self::Flat => 1,
            Self::Pack => 2,
        }
    }

    fn index_u8(self) -> u8 {
        self.index() as u8
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PackState {
    Unknown,
    Building,
    Ready,
    Faulted(PackFault),
}

impl PackState {
    fn notice(self) -> Option<&'static str> {
        match self {
            Self::Unknown | Self::Building => Some(MANIFEST_BUILDING_NOTICE),
            Self::Ready => None,
            Self::Faulted(PackFault::Missing) => Some(MANIFEST_MISSING_NOTICE),
            Self::Faulted(PackFault::Malformed) => Some(MANIFEST_MALFORMED_NOTICE),
        }
    }
}

struct Row {
    name: Box<str>,
    depth: u16,
    folder: bool,
    expanded: bool,
    path: PathBuf,
}

struct Draft {
    relative: PathBuf,
    body: String,
    stamp: Stamp,
    dirty: bool,
}

enum Body {
    Empty,
    Text,
    Image { relative: PathBuf, stamp: Stamp },
    Notice(&'static str),
}

pub struct State {
    mounts: Vec<String>,
    mount: Option<String>,
    mode: Mode,
    search: String,
    selected: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    rows: Vec<Row>,
    packs: HashMap<String, String>,
    pack_state: PackState,
    pack_pending: bool,
    draft: Option<Draft>,
    body: Body,
    notice_timer: Timer,
}

pub type Shared = Rc<RefCell<State>>;

fn matches(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let (name, query) = (name.as_bytes(), query.as_bytes());

    name.len() >= query.len() && name.windows(query.len()).any(|window| window.eq_ignore_ascii_case(query))
}

impl State {
    fn new() -> Self {
        Self {
            mounts: Vec::new(),
            mount: None,
            mode: Mode::Tree,
            search: String::new(),
            selected: None,
            expanded: HashSet::from_iter([PathBuf::new()]),
            rows: Vec::new(),
            packs: HashMap::new(),
            pack_state: PackState::Unknown,
            pack_pending: false,
            draft: None,
            body: Body::Empty,
            notice_timer: Timer::default(),
        }
    }

    fn stale(&self) -> bool {
        !self.mounts.is_empty() && self.mount.as_ref().is_some_and(|mount| !self.mounts.contains(mount))
    }

    fn writable(&self, unlocked: bool) -> bool {
        self.mount.as_deref().is_some_and(|mount| unlocked || mount != architecture::GAME)
    }

    fn reset(&mut self) {
        self.expanded = HashSet::from_iter([PathBuf::new()]);
        self.selected = None;
    }

    fn walk(&self, vfs: &Vfs, mount: &str, dir: &Path, depth: u16, query: &str, rows: &mut Vec<Row>) {
        let Some(listing) = vfs.browse(mount, dir) else {
            return;
        };

        for folder in listing.folders {
            let path = dir.join(folder.as_ref());

            if !vfs.any(mount, &path, |name| matches(name, query)) {
                continue;
            }

            let open = self.expanded.contains(&path);

            rows.push(Row { name: folder, depth, folder: true, expanded: open, path: path.clone() });

            if open {
                self.walk(vfs, mount, &path, depth + 1, query, rows);
            }
        }

        for file in listing.files {
            if !matches(&file, query) {
                continue;
            }

            let path = dir.join(file.as_ref());

            rows.push(Row { name: file, depth, folder: false, expanded: false, path });
        }
    }

    fn rebuild(&mut self, vfs: &Vfs) -> Option<&'static str> {
        self.rows.clear();

        if self.mounts.is_empty() {
            return Some(NO_MOUNTS_LABEL);
        }

        if self.stale() {
            self.selected = None;
            return Some(MISSING_LABEL);
        }

        let Some(mount) = self.mount.clone() else {
            return Some(EMPTY_LABEL);
        };
        let dropped = self.selected.as_deref().is_some_and(|path| {
            let Some(parent) = path.parent() else {
                return true;
            };
            let Some(name) = path.file_name().and_then(OsStr::to_str) else {
                return true;
            };

            !vfs.contains(&mount, parent, name)
        });

        if dropped {
            self.selected = None;
        }

        if vfs.count(&mount) == 0 {
            return Some(EMPTY_LABEL);
        }

        let query = self.search.trim().to_owned();
        let mut rows = Vec::new();

        match self.mode {
            Mode::Tree => self.walk(vfs, &mount, Path::new(""), 0, &query, &mut rows),
            Mode::Flat => {
                let mut keys = vfs.keys(&mount);

                keys.sort_unstable();

                for name in keys {
                    if !matches(&name, &query) {
                        continue;
                    }

                    if let Some(path) = vfs.stored(&mount, &name) {
                        rows.push(Row { name, depth: 0, folder: false, expanded: false, path });
                    }
                }
            }
            Mode::Pack => {
                let mut grouped: BTreeMap<&str, Vec<Box<str>>> = BTreeMap::new();
                let mut roots: Vec<Box<str>> = Vec::new();

                for name in vfs.keys(&mount) {
                    match self.packs.get(name.as_ref()) {
                        Some(pack) if !pack.is_empty() => grouped.entry(pack.as_str()).or_default().push(name),
                        _ => roots.push(name),
                    }
                }

                roots.sort_unstable();

                for (pack, mut files) in grouped {
                    files.sort_unstable();

                    if !files.iter().any(|file| matches(file, &query)) {
                        continue;
                    }

                    let open = self.expanded.contains(Path::new(pack));

                    rows.push(Row { name: pack.into(), depth: 0, folder: true, expanded: open, path: PathBuf::from(pack) });

                    if !open {
                        continue;
                    }

                    for file in files {
                        if matches(&file, &query)
                            && let Some(path) = vfs.stored(&mount, &file)
                        {
                            rows.push(Row { name: file, depth: 1, folder: false, expanded: false, path });
                        }
                    }
                }

                for file in roots {
                    if matches(&file, &query)
                        && let Some(path) = vfs.stored(&mount, &file)
                    {
                        rows.push(Row { name: file, depth: 0, folder: false, expanded: false, path });
                    }
                }
            }
        }

        let empty = rows.is_empty();

        self.rows = rows;
        empty.then_some(EMPTY_LABEL)
    }

    fn resolve(&self, vfs: &Vfs, relative: &Path) -> Option<PathBuf> {
        vfs.root(self.mount.as_deref()?).map(|root| root.join(relative))
    }

    fn commit(&mut self, vfs: &Vfs, unlocked: bool) -> Option<String> {
        let writable = self.writable(unlocked);
        let draft = self.draft.as_mut()?;

        if !draft.dirty || !writable {
            return None;
        }

        let target = self.mount.as_deref().and_then(|mount| vfs.root(mount)).map(|root| root.join(&draft.relative))?;

        match preview::save(&target, draft.body.as_bytes(), draft.stamp) {
            Ok(stamp) => {
                draft.stamp = stamp;
                draft.dirty = false;
                info!(path = %target.display(), "File edits saved");

                target.file_name().map(|name| name.to_string_lossy().into_owned())
            }
            Err(err) => {
                warn!(path = %target.display(), "discarding edits: {}", err);

                None
            }
        }
    }

    fn load_body(&mut self, vfs: &Vfs) {
        let Some(relative) = self.selected.clone() else {
            self.draft = None;
            self.body = Body::Empty;
            return;
        };

        if self.draft.as_ref().is_some_and(|draft| draft.relative == relative) {
            self.body = Body::Text;
            return;
        }

        self.draft = None;

        let Some(path) = self.resolve(vfs, &relative) else {
            self.body = Body::Empty;
            return;
        };

        self.body = match preview::load(&path) {
            Preview::Image { stamp, .. } => Body::Image { relative, stamp },
            Preview::Text { body, stamp } => {
                self.draft = Some(Draft { relative, body, stamp, dirty: false });
                Body::Text
            }
            Preview::Oversized => Body::Notice(OVERSIZED_LABEL),
            Preview::Binary => Body::Notice(BINARY_LABEL),
            Preview::Unavailable => Body::Empty,
        };
    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new()));
    let files = window.global::<Files>();

    {
        let remembered = context.memory.get();
        let mut held = state.borrow_mut();

        held.mount = remembered.files_mount;
        held.mode = Mode::from_index(i32::from(remembered.files_mode));
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    files.on_mount_set(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let picked = held.borrow().mounts.get(index.max(0) as usize).cloned();

        if let Some(mount) = picked
            && held.borrow().mount.as_deref() != Some(mount.as_str())
        {
            leave(&ui, &held, &ctx);
            {
                let mut state = held.borrow_mut();

                state.mount = Some(mount.clone());
                state.reset();
            }
            ctx.memory.update(|memory| memory.files_mount = Some(mount));
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    files.on_mode_set(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mode = Mode::from_index(index);
        let pack_state = held.borrow().pack_state;

        if mode == Mode::Pack && pack_state != PackState::Ready {
            if let Some(notice) = pack_state.notice() {
                raise(&ui, &held, notice);
            }

            ui.global::<Files>().set_mode(held.borrow().mode.index());
            return;
        }

        held.borrow_mut().mode = mode;
        ctx.memory.update(|memory| memory.files_mode = mode.index_u8());
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    files.on_search_changed(move |query| {
        held.borrow_mut().search = query.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    files.on_activate(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let target = {
            let state = held.borrow();

            state.rows.get(index.max(0) as usize).map(|row| (row.folder, row.path.clone()))
        };
        let Some((folder, path)) = target else {
            return;
        };

        if folder {
            let mut state = held.borrow_mut();

            if !state.expanded.remove(&path) {
                state.expanded.insert(path);
            }
        } else {
            leave(&ui, &held, &ctx);
            held.borrow_mut().selected = Some(path);
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    files.on_text_edited(move |value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let unlocked = ctx.state.settings().files.unlock_game_mount;
        let writable = held.borrow().writable(unlocked);

        if !writable {
            raise(&ui, &held, LOCKED_NOTICE);

            if let Some(draft) = held.borrow().draft.as_ref() {
                ui.global::<Files>().set_text(draft.body.as_str().into());
            }

            return;
        }

        let mut state = held.borrow_mut();

        if let Some(draft) = state.draft.as_mut()
            && draft.body != value.as_str()
        {
            draft.body = value.to_string();
            draft.dirty = true;
            ui.global::<Files>().set_dirty(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    files.on_save(move || {
        if let Some(ui) = weak.upgrade() {
            leave(&ui, &held, &ctx);
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    files.on_upload(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let unlocked = ctx.state.settings().files.unlock_game_mount;

        if !held.borrow().writable(unlocked) {
            raise(&ui, &held, LOCKED_NOTICE);
            return;
        }

        let landing = Rc::clone(&held);

        picker::open(
            &ui,
            &ctx,
            "Select a PNG",
            &["png"],
            Box::new(move |ui, ctx, source| {
                replace_image(ui, &landing, ctx, &source);
            }),
        );
    });

    state
}

fn raise(ui: &AppWindow, state: &Shared, text: &str) {
    let files = ui.global::<Files>();

    files.set_toast(text.into());
    files.set_toast_shown(true);

    let weak = ui.as_weak();

    state.borrow().notice_timer.start(TimerMode::SingleShot, Duration::from_millis(NOTICE_MS), move || {
        if let Some(ui) = weak.upgrade() {
            ui.global::<Files>().set_toast_shown(false);
        }
    });
}

pub fn open(ui: &AppWindow, state: &Shared, context: &Rc<Context>, mount: String, relative: PathBuf) {
    leave(ui, state, context);

    {
        let mut held = state.borrow_mut();

        held.mount = Some(mount.clone());
        held.reset();
        held.draft = None;
        held.search.clear();
        held.expanded.extend(relative.ancestors().skip(1).map(Path::to_path_buf));
        held.selected = Some(relative);

        if held.mode == Mode::Pack {
            held.mode = Mode::Tree;
        }
    }

    let files = ui.global::<Files>();

    files.set_search("".into());
    files.set_list_open(false);
    context.memory.update(|memory| memory.files_mount = Some(mount));
}

pub fn leave(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let vault = context.engine.vault();
    let unlocked = context.state.settings().files.unlock_game_mount;
    let saved = state.borrow_mut().commit(&vault.vfs, unlocked);

    if let Some(name) = saved {
        context.changed(ui, &name);
    }
}

fn replace_image(ui: &AppWindow, state: &Shared, context: &Rc<Context>, source: &Path) {
    let vault = context.engine.vault();
    let Ok(bytes) = std::fs::read(source).inspect_err(|err| warn!(path = %source.display(), "upload read failed: {}", err)) else {
        return;
    };

    if !preview::is_png(&bytes) {
        warn!(path = %source.display(), "refusing to upload a file that is not a PNG");
        raise(ui, state, "Only PNG images can replace a file");
        return;
    }

    let outcome = {
        let held = state.borrow();
        let Body::Image { relative, stamp } = &held.body else {
            return;
        };
        let Some(target) = held.resolve(&vault.vfs, relative) else {
            return;
        };

        preview::save(&target, &bytes, *stamp).map(|_| target)
    };

    match outcome {
        Ok(target) => {
            info!(path = %target.display(), "Image replaced");
            context.images.forget_all();
            refresh(ui, state, context);
        }
        Err(err) => {
            warn!("upload failed: {}", err);
            raise(ui, state, "The image could not be written");
        }
    }
}

pub fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let vault = context.engine.vault();
    let keys: Vec<String> = vault.vfs.mounted().iter().map(|key| key.to_string()).collect();

    {
        let mut held = state.borrow_mut();

        if keys != held.mounts {
            held.mounts = keys;
        }

        if held.stale() {
            info!("Persisted mount is no longer indexed, falling back to the first mount");
            held.mount = None;
            held.selected = None;
        }

        if held.mount.is_none() {
            held.mount = held.mounts.first().cloned();
            held.reset();
        }
    }

    probe_packs(ui, state, context);
    refresh(ui, state, context);
}

fn probe_packs(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    {
        let mut held = state.borrow_mut();

        if held.pack_pending {
            return;
        }

        held.pack_pending = true;
        held.pack_state = PackState::Building;
    }

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    bridge::offload(ui, "pack_index", import::pack_index, move |ui, result| {
        {
            let mut held = landing.borrow_mut();

            held.pack_pending = false;

            match result {
                Ok(index) => {
                    held.packs = index;
                    held.pack_state = PackState::Ready;
                }
                Err(fault) => {
                    held.packs.clear();
                    held.pack_state = PackState::Faulted(fault);

                    if held.mode == Mode::Pack {
                        info!("Pack view is unavailable, falling back to the tree view");
                        held.mode = Mode::Tree;
                    }
                }
            }
        }

        if ui.global::<Shell>().get_page() == Page::Files {
            refresh(ui, &landing, &ctx);
        }
    });
}

pub fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let files = ui.global::<Files>();
    let vault = context.engine.vault();
    let unlocked = context.state.settings().files.unlock_game_mount;
    let mut held = state.borrow_mut();
    let empty = held.rebuild(&vault.vfs);

    held.load_body(&vault.vfs);

    let selected = held.selected.clone();
    let rows: Vec<FileRow> = held
        .rows
        .iter()
        .map(|row| FileRow { name: row.name.as_ref().into(), depth: i32::from(row.depth), folder: row.folder, expanded: row.expanded, selected: !row.folder && selected.as_deref() == Some(row.path.as_path()) })
        .collect();
    let mount_index = held.mount.as_ref().and_then(|mount| held.mounts.iter().position(|held| held == mount)).map_or(-1, |index| index as i32);
    let header = held
        .mount
        .as_deref()
        .zip(held.selected.as_deref().and_then(|path| path.file_name()).and_then(OsStr::to_str))
        .map_or_else(|| HEADER_EMPTY.to_owned(), |(mount, name)| format!("{mount}{HEADER_SEPARATOR}{name}"));

    files.set_mounts(ModelRc::new(VecModel::from(held.mounts.iter().map(|mount| SharedString::from(mount.as_str())).collect::<Vec<_>>())));
    files.set_mount(mount_index);
    files.set_stale(held.stale());
    files.set_mode(held.mode.index());
    files.set_rows(ModelRc::new(VecModel::from(rows)));
    files.set_empty_label(empty.unwrap_or_default().into());
    files.set_header(header.into());
    files.set_writable(held.writable(unlocked));

    match &held.body {
        Body::Empty => files.set_body(0),
        Body::Text => {
            files.set_body(1);

            if let Some(draft) = &held.draft {
                files.set_text(draft.body.as_str().into());
                files.set_dirty(draft.dirty);
            }
        }
        Body::Image { relative, .. } => {
            let picture = held.resolve(&vault.vfs, relative).and_then(|path| std::fs::read(&path).ok()).and_then(|bytes| image::load_from_memory(&bytes).ok()).map(|decoded| images::to_slint(&decoded.to_rgba8()));

            files.set_body(2);
            files.set_picture(picture.unwrap_or_default());
        }
        Body::Notice(text) => {
            files.set_body(3);
            files.set_notice((*text).into());
        }
    }
}
