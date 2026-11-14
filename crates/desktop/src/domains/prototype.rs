mod panel;
mod tree;

use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::alignment::Vertical;
use iced::{Element, Font, Length, Padding, Size, Task, Theme};
use codex::graphics::backdrop;
use tracing::warn;

use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::prototype::{self as effects, Asset, Document, Entry, Shape, Slot, Spot, FIELDS};
use foundry::domains::utilities::background::{self as builder, Loaded, Overrides, DEFAULT_SEED};
use foundry::Vault;

use crate::app::state::BackgroundState;
use crate::app::theme;
use crate::common::feedback::{self, Slot as Latch};
use crate::common::row_window::{self, RowWindow};
use crate::common::{fonts, glyphs};
use crate::systems::background as viewer;
use crate::widget::{branches, list_row, open_mark, smooth_scroll, toast, Guide, Tone, Tracer};

use tree::TreeRow;

const PANEL_WIDTH: f32 = 372.0;
const PANEL_PADDING: f32 = 6.0;
const BODY_PADDING: f32 = 6.0;
const GAP: f32 = 6.0;
const ROW_GAP: f32 = 3.0;

const CLOSE_LABEL: &str = "\u{00d7}";
const CONFIRM_MARK: &str = "?";

const LABEL_SIZE: f32 = 12.0;
const CELL_SIZE: f32 = 12.0;
const CELL_PADDING: f32 = 3.0;
const INDEX_WIDTH: f32 = 22.0;
const ROW_HEIGHT: f32 = 21.0;
const ROW_SPACING: f32 = 0.0;
const ROW_PADDING: f32 = 6.0;
const INDENT: f32 = 11.0;

const TREE_TEXT_SIZE: f32 = 11.0;
const CHAR_WIDTH: f32 = TREE_TEXT_SIZE * 0.6;

const MARKER_SIZE: f32 = ROW_HEIGHT * fonts::TRIANGLE_ROW;
const MARKER_LINE_HEIGHT: f32 = ROW_HEIGHT / MARKER_SIZE;
const MARKER_WIDTH: f32 = 14.0;

const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 2.0;
const SCROLLBAR_ALLOWANCE: f32 = SCROLLBAR_WIDTH + SCROLLBAR_MARGIN * 2.0;
const SCROLL_TAIL: f32 = 14.0;
const KEY_ROW_PAD: f32 = 3.0;
const KEY_ROW_INSET: f32 = 2.0;
const KEY_HEAD_HEIGHT: f32 = 19.0;
const PAGE_DIALS: usize = 8;
const PAGE_BACK: &str = "\u{25c2}";
const PAGE_NEXT: &str = "\u{25b8}";
const PAGER_STEP: f32 = 26.0;
const DEBUG_WIDTH: f32 = 78.0;
const OPTION_WIDTH: f32 = 176.0;
const COMBO_WIDTH: f32 = 88.0;

const FOLDER_OPEN: &str = "\u{25be}";
const FOLDER_SHUT: &str = "\u{25b8}";
const DROP_WIDTH: f32 = 18.0;
const STEP_WIDTH: f32 = 36.0;
const BASE_WIDTH: f32 = 112.0;
const SHAPE_WIDTH: f32 = 66.0;
const FACT_ROW_HEIGHT: f32 = 21.0;
const FACT_ROW_PAD: f32 = 4.0;
const FACT_LABEL: f32 = 62.0;
const LOOP_CARD_PAD: f32 = 3.0;
const HEAD_HEIGHT: f32 = 22.0;
const FIELD_ROW_HEIGHT: f32 = 24.0;
const BOARD_HEIGHT: f32 = 208.0;

const NOTICE_EXPIRY: Duration = Duration::from_secs(6);
const HISTORY_DEPTH: usize = 25;
const COALESCE: Duration = Duration::from_millis(700);
const SEED_CEILING: u64 = i32::MAX as u64;
const NUMBER_GLYPHS: &str = "-+.eE";
const SEPARATOR: &str = " \u{00b7} ";
const RANGE_MARK: &str = " ~ ";

const NO_FILE_NOTICE: &str = "No effects file is loaded";
const NO_FILE_HINT: &str = "Pick an effects file under Option";
const NO_EMITTERS_NOTICE: &str = "This file declares no emitters";
const OFF_NOTICE: &str = "This parameter is off";
const BORROW_NOTICE: &str = "This file borrows another background's effects";
const DIRTY_NOTICE: &str = "This file has changes that are not exported\nExport or revert it before opening another";
const MISSING_NOTICE: &str = "That effects file could not be read";
const TARGET_NOTICE: &str = "The borrowed background has no readable effects";
const NO_MOD_NOTICE: &str = "No mod is enabled\nEnable one under Mods to export into it";
const WRITE_FAILED_NOTICE: &str = "The last export could not be written";
const NO_HOST_NOTICE: &str = "No background draws these effects, so there is nothing to preview on";
const LOADING_NOTICE: &str = "Loading background\u{2026}";
const SEED_HINT: &str = "100";
const CELL_HINT: &str = "0";

#[derive(Clone)]
pub struct Fresh(Arc<Loaded>);

impl fmt::Debug for Fresh {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Fresh({})", self.0.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pick {
    #[default]
    Nothing,
    Emitter(usize),
    Field(usize, usize),
}

impl Pick {
    fn emitter(self) -> Option<usize> {
        match self {
            Self::Emitter(at) | Self::Field(at, _) => Some(at),
            Self::Nothing => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Redirect,
    Name,
    Asset(Asset),
    Cell(Slot),
    Value(usize),
    Spot(Spot),
    Extra(String),
}

impl Address {
    fn numeric(&self) -> bool {
        matches!(self, Self::Redirect | Self::Cell(_) | Self::Value(_) | Self::Spot(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dial {
    Effects,
    Preview,
    Seed,
}

impl Dial {
    const ALL: [Self; 3] = [Self::Effects, Self::Preview, Self::Seed];

    fn label(self) -> &'static str {
        match self {
            Self::Effects => "Effects",
            Self::Preview => "Preview",
            Self::Seed => "Seed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    Emitter(usize),
    Value(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Scanned(u64, Vec<Entry>),
    Open(Entry),
    Host(u32),
    Cycle(Dial),
    Seed(String),
    Loaded(u64, Result<Fresh, String>),
    Viewer(viewer::Message),
    Row(usize),
    Scrolled(f32),
    Typed(Address, String),
    Shaped(Shape),
    Based(Slot, &'static str),
    SpotBased(&'static str),
    Spaced(bool),
    AddEmitter,
    CopyEmitter,
    Shift(bool),
    AddValue,
    Drop(Dropped),
    DropExpired,
    Follow,
    Detach,
    Undo,
    Revert,
    Export,
    ExportExpired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tag {
    Typed(Address),
    Shape,
    Base,
    Spacing,
    Emitters,
    Values,
    Bulk,
}

struct Shot {
    document: Document,
    picked: Pick,
}

struct Mark {
    tag: Tag,
    picked: Pick,
    at: Instant,
}

#[derive(Default)]
struct History {
    shots: Vec<Shot>,
    last: Option<Mark>,
}

impl History {
    fn push(&mut self, document: Document, picked: Pick, tag: Tag) {
        let merged = tag != Tag::Bulk
            && self.last.as_ref().is_some_and(|mark| mark.tag == tag && mark.picked == picked && mark.at.elapsed() < COALESCE);

        self.last = Some(Mark { tag, picked, at: Instant::now() });

        if merged {
            return;
        }

        if self.shots.len() == HISTORY_DEPTH {
            self.shots.remove(0);
        }

        self.shots.push(Shot { document, picked });
    }

    fn pop(&mut self) -> Option<Shot> {
        self.last = None;
        self.shots.pop()
    }

    fn clear(&mut self) {
        self.last = None;
        self.shots.clear();
    }
}

struct Origin {
    id: u32,
    name: String,
    game: Option<PathBuf>,
    read_from: PathBuf,
    stamp: Stamp,
    saved: Vec<u8>,
}

struct Draft {
    address: Address,
    text: String,
}

struct Sheet {
    redirect: Option<String>,
    name: String,
    assets: Option<[String; 4]>,
    spaced: bool,
    spots: [String; 4],
    extras: Vec<(String, String)>,
    shape: Shape,
    cells: [String; 8],
    values: Vec<String>,
    facts: [(&'static str, String); 8],
}

impl Default for Sheet {
    fn default() -> Self {
        Self {
            redirect: None,
            name: String::new(),
            assets: None,
            spaced: false,
            spots: Default::default(),
            extras: Vec::new(),
            shape: Shape::Off,
            cells: Default::default(),
            values: Vec::new(),
            facts: Sheet::facts(None, 0),
        }
    }
}

impl Sheet {
    fn facts(document: Option<&Document>, at: usize) -> [(&'static str, String); 8] {
        let asset = |asset: Asset| document.and_then(|document| document.asset(at, asset)).unwrap_or_default();
        let brief = |key: &str| document.map_or_else(String::new, |document| tree::summary(document, at, key));
        let named = document.filter(|document| at < document.len()).map_or_else(String::new, |document| tree::emitter_label(document, at));

        [
            ("Emitter", named),
            ("Sheet", asset(Asset::Image)),
            ("Cuts", asset(Asset::Imgcut)),
            ("Model", asset(Asset::Model)),
            ("Motion", asset(Asset::Anime)),
            ("Count", brief("count")),
            ("Layer", brief("z")),
            ("Life", brief("lifeTime")),
        ]
    }

    fn of(document: &Document, picked: Pick) -> Self {
        let at = picked.emitter().unwrap_or(0);
        let key = match picked {
            Pick::Field(_, index) => FIELDS.get(index).map(|field| field.key),
            _ => None,
        };

        Self {
            redirect: document.redirect(),
            name: document.name(at),
            assets: document.asset(at, Asset::Image).map(|_| Asset::ALL.map(|asset| document.asset(at, asset).unwrap_or_default())),
            spaced: document.spaced(at),
            spots: Spot::ALL.map(|spot| document.spot(at, spot)),
            extras: document.extras(at),
            shape: key.map_or(Shape::Off, |key| document.shape(at, key)),
            cells: Slot::ALL.map(|slot| key.map_or_else(String::new, |key| document.cell(at, key, slot))),
            values: key.map_or_else(Vec::new, |key| document.values(at, key)),
            facts: Self::facts(picked.emitter().map(|_| document), at),
        }
    }

    fn cell(&self, slot: Slot) -> &str {
        Slot::ALL.iter().position(|held| *held == slot).map_or("", |index| self.cells[index].as_str())
    }

    fn spot(&self, spot: Spot) -> &str {
        Spot::ALL.iter().position(|held| *held == spot).map_or("", |index| self.spots[index].as_str())
    }
}

#[derive(Default)]
pub struct State {
    entries: Vec<Entry>,
    scanned: bool,
    scanning: bool,
    scan_run: u64,
    load_run: u64,
    loading: Option<u64>,
    origin: Option<Origin>,
    document: Option<Document>,
    picked: Pick,
    expanded: HashSet<usize>,
    rows: Vec<TreeRow>,
    widest: f32,
    scroll: f32,
    sheet: Sheet,
    dirty: bool,
    draft: Option<Draft>,
    history: History,
    confirm: Latch<Dropped>,
    exported: Latch<bool>,
    hosts: Vec<u32>,
    host: Option<u32>,
    seed_input: String,
    notice_text: String,
    raised: bool,
    flash: Option<Instant>,
    watched: BTreeSet<String>,
    viewer: viewer::State,
}

fn is_effects_file(name: &str) -> bool {
    name.starts_with("bg") && name.ends_with(".json")
}

fn stepped<T: Copy + PartialEq>(all: &[T], held: Option<T>) -> Option<T> {
    let next = held.and_then(|held| all.iter().position(|item| *item == held)).map_or(0, |at| at + 1);

    all.get(next % all.len().max(1)).copied()
}

impl State {
    pub(crate) fn pace(&self) -> Duration {
        self.viewer.pace()
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.history.shots.is_empty()
    }

    fn seed(&self) -> u32 {
        self.seed_input.parse::<u32>().ok().filter(|seed| *seed > 0).unwrap_or(DEFAULT_SEED)
    }

    fn opened(&self) -> Option<Entry> {
        let id = self.origin.as_ref()?.id;

        self.entries.iter().find(|entry| entry.id == id).copied()
    }

    fn raise(&mut self, text: &str) {
        self.notice_text = text.to_owned();
        self.raised = true;
        self.flash = Some(Instant::now());
    }

    pub(crate) fn enter(&mut self, vault: &Arc<Vault>) -> Task<Message> {
        if self.scanned || self.scanning {
            return Task::none();
        }

        self.scan(vault)
    }

    fn scan(&mut self, vault: &Arc<Vault>) -> Task<Message> {
        let vault = Arc::clone(vault);

        self.scan_run = self.scan_run.wrapping_add(1);
        self.scanning = true;

        let run = self.scan_run;

        Task::perform(smol::unblock(move || effects::catalog(&vault.vfs)), move |entries| Message::Scanned(run, entries))
    }

    fn rehost(&mut self) {
        let Some(origin) = &self.origin else {
            self.hosts.clear();
            self.host = None;

            return;
        };

        self.hosts = effects::hosts(&self.entries, origin.id);

        if !self.host.is_some_and(|host| self.hosts.contains(&host)) {
            self.host = self.hosts.first().copied();
        }
    }

    fn resolve(&mut self) {
        let count = self.document.as_ref().map_or(0, Document::len);

        self.picked = match self.picked {
            _ if count == 0 => Pick::Nothing,
            Pick::Nothing => Pick::Emitter(0),
            Pick::Emitter(at) => Pick::Emitter(at.min(count - 1)),
            Pick::Field(at, index) if at < count => Pick::Field(at, index),
            Pick::Field(..) => Pick::Emitter(count - 1),
        };
        self.expanded.retain(|at| *at < count);
        self.rows = self.document.as_ref().map_or_else(Vec::new, |document| tree::listing(document, &self.expanded));
        self.widest = self.rows.iter().map(TreeRow::span).fold(0.0, f32::max);
        self.sheet = self.document.as_ref().map_or_else(Sheet::default, |document| Sheet::of(document, self.picked));
        self.dirty = match (&self.document, &self.origin) {
            (Some(document), Some(origin)) => document.bytes() != origin.saved,
            _ => false,
        };
    }

    fn open(&mut self, id: u32, vault: &Arc<Vault>) -> Task<Message> {
        let name = backdrop::effects_name(id);
        let read = vault.vfs.find(name.as_str()).and_then(|path| {
            let bytes = vault.vfs.read(&path).ok()?;
            let document = Document::parse(bytes.as_ref()).ok()?;
            let stamp = preview::stamp_of(&vault.vfs.source(&path)).unwrap_or_default();

            Some((path, bytes, document, stamp))
        });
        let Some((path, bytes, document, stamp)) = read else {
            self.raise(MISSING_NOTICE);

            return Task::none();
        };

        if !self.origin.as_ref().is_some_and(|origin| origin.id == id) {
            self.picked = Pick::Nothing;
            self.expanded.clear();
            self.scroll = 0.0;
        }

        self.origin = Some(Origin { id, game: vault.vfs.pristine(name.as_str()), name, read_from: path, stamp, saved: bytes.to_vec() });
        self.document = Some(document);
        self.draft = None;
        self.history.clear();
        self.confirm.clear();
        self.resolve();
        self.rehost();

        self.preview(vault)
    }

    fn preview(&mut self, vault: &Arc<Vault>) -> Task<Message> {
        let (Some(document), Some(host)) = (&self.document, self.host) else {
            self.loading = None;
            self.viewer.clear();

            return Task::none();
        };
        let overrides = Overrides { effects: Some(Arc::from(document.bytes())), ..Overrides::default() };
        let vault = Arc::clone(vault);

        self.load_run = self.load_run.wrapping_add(1);

        let run = self.load_run;

        self.loading = Some(run);

        Task::perform(
            smol::unblock(move || builder::load(&vault.vfs, host, &overrides).map(|loaded| Fresh(Arc::new(loaded)))),
            move |outcome| Message::Loaded(run, outcome),
        )
    }

    fn changed(&mut self, vault: &Arc<Vault>) -> Task<Message> {
        self.resolve();

        self.preview(vault)
    }

    fn edit(&mut self, tag: Tag, vault: &Arc<Vault>, apply: impl FnOnce(&mut Document) -> bool) -> Task<Message> {
        let Some(document) = self.document.as_mut() else {
            return Task::none();
        };
        let before = document.clone();

        if !apply(document) {
            return Task::none();
        }

        self.history.push(before, self.picked, tag);
        self.changed(vault)
    }

    fn typed(&mut self, address: Address, text: String, vault: &Arc<Vault>) -> Task<Message> {
        let listed = matches!(address, Address::Cell(Slot::Values));

        if address.numeric() && !text.chars().all(|glyph| glyph.is_ascii_digit() || NUMBER_GLYPHS.contains(glyph) || (listed && matches!(glyph, ',' | ' '))) {
            return Task::none();
        }

        let at = self.picked.emitter().unwrap_or(0);
        let field = match self.picked {
            Pick::Field(_, index) => FIELDS.get(index),
            _ => None,
        };
        let typed = text.clone();
        let target = address.clone();

        self.draft = Some(Draft { address: address.clone(), text });

        self.edit(Tag::Typed(address), vault, move |document| match (&target, field) {
            (Address::Redirect, _) => document.set_redirect(typed.trim()),
            (Address::Name, _) => {
                document.set_name(at, &typed);

                true
            }
            (Address::Asset(asset), _) => {
                document.set_asset(at, *asset, &typed);

                true
            }
            (Address::Cell(slot), Some(field)) => document.set_cell(at, field.key, field.kind, *slot, &typed),
            (Address::Value(index), Some(field)) => document.set_value(at, field.key, field.kind, *index, &typed),
            (Address::Spot(spot), _) => document.set_spot(at, *spot, &typed),
            (Address::Extra(key), _) => document.set_extra(at, key, &typed),
            (Address::Cell(_) | Address::Value(_), None) => false,
        })
    }

    fn save(&mut self, vault: &Arc<Vault>, target: &str) -> bool {
        let (Some(document), Some(origin)) = (&self.document, self.origin.as_mut()) else {
            return false;
        };
        let staged = if origin.game.as_ref() != Some(&origin.read_from) {
            Some((origin.read_from.clone(), origin.stamp))
        } else {
            mods::ensure_as(&vault.vfs, target, &origin.read_from, &origin.name)
                .inspect_err(|err| warn!(file = origin.name, "Prototype editor could not stage the file: {}", err))
                .ok()
                .and_then(|path| {
                    if let Err(err) = vault.vfs.create((target, path.as_path())) {
                        warn!(path = %path.display(), "Prototype editor staged a file the index would not take: {}", err);
                    }

                    preview::stamp(&path).map(|stamp| (path, stamp))
                })
        };
        let Some((path, stamp)) = staged else {
            return false;
        };
        let body = document.bytes();

        match preview::save(&path, &body, stamp) {
            Ok(fresh) => {
                origin.read_from = path;
                origin.stamp = fresh;
                origin.saved = body;
                self.draft = None;
                self.scanned = false;
                self.resolve();

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Prototype editor could not write the file: {}", err);

                false
            }
        }
    }

    pub(crate) fn update(&mut self, message: Message, vault: &Arc<Vault>, prefs: &mut BackgroundState, active_mod: Option<&str>) -> Task<Message> {
        match message {
            Message::Tick => {
                let scan = if self.scanned || self.scanning { Task::none() } else { self.scan(vault) };

                if self.raised && self.flash.is_none_or(|at| at.elapsed() >= NOTICE_EXPIRY) {
                    self.raised = false;
                }

                self.viewer.tick();
                scan
            }
            Message::Scanned(run, entries) => {
                if run != self.scan_run {
                    return Task::none();
                }

                self.scanning = false;
                self.scanned = true;
                self.entries = entries;

                let held = self.host;

                self.rehost();

                if self.host == held {
                    return Task::none();
                }

                self.preview(vault)
            }
            Message::Open(entry) => {
                if self.dirty {
                    self.raise(DIRTY_NOTICE);

                    return Task::none();
                }

                self.open(entry.id, vault)
            }
            Message::Host(host) => {
                self.host = Some(host);
                self.preview(vault)
            }
            Message::Cycle(Dial::Effects) => match stepped(&self.entries, self.opened()) {
                Some(entry) => self.update(Message::Open(entry), vault, prefs, active_mod),
                None => Task::none(),
            },
            Message::Cycle(Dial::Preview) => match stepped(&self.hosts, self.host) {
                Some(host) => self.update(Message::Host(host), vault, prefs, active_mod),
                None => Task::none(),
            },
            Message::Cycle(Dial::Seed) => self.update(Message::Seed(String::new()), vault, prefs, active_mod),
            Message::Seed(value) => {
                if !value.chars().all(|glyph| glyph.is_ascii_digit()) {
                    return Task::none();
                }

                if !value.is_empty() && !value.parse::<u64>().is_ok_and(|seed| (1..=SEED_CEILING).contains(&seed)) {
                    return Task::none();
                }

                self.seed_input = value;
                self.viewer.set_seed(self.seed());
                Task::none()
            }
            Message::Loaded(run, outcome) => {
                if self.loading != Some(run) {
                    return Task::none();
                }

                self.loading = None;

                match outcome {
                    Ok(Fresh(loaded)) => {
                        let loaded = Arc::unwrap_or_clone(loaded);

                        self.watched = loaded.watched.clone();
                        self.viewer.revise(loaded, self.seed(), prefs);
                    }
                    Err(problem) => {
                        warn!("Prototype preview could not be built: {}", problem);
                        self.viewer.clear();
                    }
                }

                Task::none()
            }
            Message::Viewer(msg) => self.viewer.update(msg, prefs).map(Message::Viewer),
            Message::Row(index) => {
                let Some(row) = self.rows.get(index) else {
                    return Task::none();
                };

                self.picked = match row.field {
                    Some(field) => Pick::Field(row.emitter, field),
                    None => {
                        if !self.expanded.remove(&row.emitter) {
                            self.expanded.insert(row.emitter);
                        }

                        Pick::Emitter(row.emitter)
                    }
                };
                self.draft = None;
                self.confirm.clear();
                self.resolve();
                Task::none()
            }
            Message::Scrolled(offset) => {
                self.scroll = offset;
                Task::none()
            }
            Message::Typed(address, text) => self.typed(address, text, vault),
            Message::Shaped(shape) => {
                let Pick::Field(at, index) = self.picked else {
                    return Task::none();
                };
                let Some(field) = FIELDS.get(index) else {
                    return Task::none();
                };

                self.draft = None;
                self.edit(Tag::Shape, vault, |document| {
                    let held = document.shape(at, field.key);

                    document.reshape(at, field.key, shape);
                    held != shape
                })
            }
            Message::Based(slot, name) => {
                let Pick::Field(at, index) = self.picked else {
                    return Task::none();
                };
                let Some(field) = FIELDS.get(index) else {
                    return Task::none();
                };

                self.edit(Tag::Base, vault, |document| document.set_cell(at, field.key, field.kind, slot, name))
            }
            Message::SpotBased(name) => {
                let at = self.picked.emitter().unwrap_or(0);

                self.edit(Tag::Base, vault, |document| document.set_spot(at, Spot::Base, name))
            }
            Message::Spaced(on) => {
                let at = self.picked.emitter().unwrap_or(0);

                self.draft = None;
                self.edit(Tag::Spacing, vault, |document| {
                    let held = document.spaced(at);

                    document.set_spaced(at, on);
                    held != on
                })
            }
            Message::AddEmitter => self.restructure(vault, |document, _| document.append()),
            Message::CopyEmitter => self.restructure(vault, |document, at| document.duplicate(at)),
            Message::Shift(earlier) => self.restructure(vault, move |document, at| document.shift(at, earlier)),
            Message::AddValue => {
                let Pick::Field(at, index) = self.picked else {
                    return Task::none();
                };
                let Some(field) = FIELDS.get(index) else {
                    return Task::none();
                };

                self.edit(Tag::Values, vault, |document| document.add_value(at, field.key))
            }
            Message::Drop(dropped) => {
                if !self.confirm.take(&dropped) {
                    return self.confirm.set(dropped, Message::DropExpired);
                }

                self.draft = None;

                match (dropped, self.picked) {
                    (Dropped::Emitter(at), _) => self.restructure(vault, move |document, _| document.remove(at).then(|| at.saturating_sub(1))),
                    (Dropped::Value(entry), Pick::Field(at, index)) => match FIELDS.get(index) {
                        Some(field) => self.edit(Tag::Values, vault, |document| document.drop_value(at, field.key, entry)),
                        None => Task::none(),
                    },
                    (Dropped::Value(_), _) => Task::none(),
                }
            }
            Message::DropExpired => {
                self.confirm.expire();
                Task::none()
            }
            Message::Follow => {
                if self.dirty {
                    self.raise(DIRTY_NOTICE);

                    return Task::none();
                }

                let Some(target) = self.document.as_ref().and_then(Document::redirect_id) else {
                    return Task::none();
                };

                self.open(target, vault)
            }
            Message::Detach => {
                let Some(id) = self.document.as_ref().and_then(Document::redirect_id) else {
                    return Task::none();
                };
                let target = effects::settled(&self.entries, id);
                let Some(source) = effects::read(&vault.vfs, target).filter(|source| source.redirect().is_none()) else {
                    self.raise(TARGET_NOTICE);

                    return Task::none();
                };

                self.draft = None;
                self.edit(Tag::Bulk, vault, |document| {
                    document.adopt(&source);

                    true
                })
            }
            Message::Undo => {
                let Some(shot) = self.history.pop() else {
                    return Task::none();
                };

                self.document = Some(shot.document);
                self.picked = shot.picked;
                self.draft = None;
                self.confirm.clear();
                self.changed(vault)
            }
            Message::Revert => {
                let Some(id) = self.origin.as_ref().map(|origin| origin.id) else {
                    return Task::none();
                };

                self.open(id, vault)
            }
            Message::Export => {
                let Some(target) = active_mod else {
                    self.raise(NO_MOD_NOTICE);

                    return Task::none();
                };
                let placed = self.save(vault, target);

                if !placed {
                    self.raise(WRITE_FAILED_NOTICE);
                }

                self.exported.set(placed, Message::ExportExpired)
            }
            Message::ExportExpired => {
                self.exported.expire();
                Task::none()
            }
        }
    }

    fn restructure(&mut self, vault: &Arc<Vault>, apply: impl FnOnce(&mut Document, usize) -> Option<usize>) -> Task<Message> {
        let at = self.picked.emitter().unwrap_or(0);
        let mut moved = None;
        let task = self.edit(Tag::Emitters, vault, |document| {
            moved = apply(document, at);
            moved.is_some()
        });

        if let Some(to) = moved {
            self.picked = Pick::Emitter(to);
            self.expanded.clear();
            self.draft = None;
            self.confirm.clear();
            self.resolve();
        }

        task
    }

    pub(crate) fn files_changed(&mut self, paths: &[PathBuf], vault: &Arc<Vault>) -> Task<Message> {
        let names: Vec<String> = paths.iter().filter_map(|path| path.file_name().and_then(|name| name.to_str())).map(str::to_ascii_lowercase).collect();

        if names.iter().any(|name| is_effects_file(name)) {
            self.scanned = false;
        }

        let Some(origin) = &self.origin else {
            return Task::none();
        };

        if !self.dirty && names.contains(&origin.name) {
            let current = vault.vfs.find(origin.name.as_str()).and_then(|path| vault.vfs.read(&path).ok());

            if current.is_some_and(|bytes| bytes.as_ref() != origin.saved.as_slice()) {
                return self.open(origin.id, vault);
            }
        }

        if !names.iter().any(|name| self.watched.contains(name)) {
            return Task::none();
        }

        self.preview(vault)
    }

    pub(crate) fn invalidate(&mut self, vault: &Arc<Vault>) -> Task<Message> {
        self.scanned = false;

        match self.origin.as_ref().map(|origin| origin.id) {
            Some(id) if !self.dirty => self.open(id, vault),
            Some(_) => self.preview(vault),
            None => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{stepped, Document, History, Pick, Tag, HISTORY_DEPTH};

    fn doc() -> Document {
        Document::parse(r#"{"data":[{"name":"a","count":{"value":1}}]}"#).unwrap()
    }

    #[test]
    fn typing_in_one_cell_is_one_undo_step() {
        let mut history = History::default();

        history.push(doc(), Pick::Emitter(0), Tag::Shape);
        history.push(doc(), Pick::Emitter(0), Tag::Shape);
        assert_eq!(history.shots.len(), 1);

        history.push(doc(), Pick::Field(0, 0), Tag::Shape);
        history.push(doc(), Pick::Field(0, 0), Tag::Base);
        assert_eq!(history.shots.len(), 3);
    }

    #[test]
    fn bulk_steps_never_merge_and_the_oldest_falls_off() {
        let mut history = History::default();

        for _ in 0..HISTORY_DEPTH + 5 {
            history.push(doc(), Pick::Nothing, Tag::Bulk);
        }

        assert_eq!(history.shots.len(), HISTORY_DEPTH);
        assert!(history.pop().is_some());
        assert!(history.last.is_none());
    }

    #[test]
    fn a_dial_steps_to_the_next_option_and_wraps() {
        assert_eq!(stepped(&[3, 5, 9], Some(5)), Some(9));
        assert_eq!(stepped(&[3, 5, 9], Some(9)), Some(3));
        assert_eq!(stepped(&[3, 5, 9], None), Some(3));
        assert_eq!(stepped::<u32>(&[], None), None);
    }
}
