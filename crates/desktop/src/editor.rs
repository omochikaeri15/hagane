mod prose;
mod artwork;
mod audio;
mod captions;
mod catfruit;
mod finder;
mod evolve;
mod filter;
mod gift;
mod group;
mod ledger;
mod maplayout;
mod cuts;
mod dungeon;
mod logins;
mod unused;
mod preset;
mod records;
mod roster;
mod sheet;
mod treasure;
mod ground;
mod menu;
mod registry;
mod figures;
mod target;
mod watch;

use std::fs;
use std::io::{self, Read};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use iced::{Element, Point, Size, Task};
use tracing::{info, trace, warn};

use foundry::domains::cat::animation as cat_animation;
use codex::combat::{Faction, Identity};
use foundry::domains::cat::files as cat_files;
use foundry::domains::enemy::animation as enemy_animation;
use foundry::common::architecture;
use foundry::domains::mods;
use foundry::domains::sandbox::base;
use foundry::domains::stage::files as stage_files;
use foundry::domains::stage::authoring as stage_authoring;

use foundry::domains::stage::{names, GlobalMapId};
use foundry::domains::settings::{ContextScope, EditorMode};
use foundry::systems::editor::tables::*;
use foundry::systems::editor::plans::*;
use foundry::systems::editor::{self as core};
use foundry::Vfs;
use codex::chapter::treasure::{data_file, STAGES as TREASURE_STAGES};
use codex::common;
use codex::graphics::tools::crash::Side;

use crate::app::state::SandboxTab;
use crate::app::{theme, HaganeApp, Page};
use crate::domains::cat::DetailTab;
use crate::domains::stage::category::CategoryExt;
use crate::domains::studio;
use foundry::domains::studio as foundry_studio;
use crate::domains::enemy::DetailTab as EnemyTab;
use crate::common::dialog;
use crate::widget::popup;
use crate::common::feedback::{Slot, NO_ACTIONS_LABEL, NO_ACTIONS_NOTICE, UNSUPPORTED_NOTICE};

pub(crate) use foundry::systems::editor::{Art, Mark, Pane, Reader};
pub(crate) use target::{deflect, suppress, target};

use prose::Framed;

pub(crate) trait Paged {
    fn page(self) -> Page;
}
pub(crate) use watch::watch;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    FileRow(usize),
    CatRow(u32),
    LineupCell(foundry::domains::sandbox::Cell),
    EnemyRow(u32),
    CatLevels,
    CatForms,
    CatTalents,
    CatCombo(usize),
    StageGround,
    StageData,
    MapDrops,
    StageDrops,
    TreasureDrop(u32),
    StageMake,
    CatAttributes,
    EnemyAttributes,
    CatAnimation,
    EnemyAnimation,
    AnimPart(usize),
    AnimCurve(usize),
    AnimFields,
    CatIcon,
    EnemyIcon,
    MapBanner,
    StageBanner,
    MapName,
    StageName,
    MapRow(u32),
    StageRow(u32),
    CatExplanation,
    EnemyName,
    EnemyDescription,
    Ability(Identity, Faction),
}

pub(crate) struct Context {
    values: EditorMode,
    page: Page,
    seat: Seat,
    file: Option<FileTarget>,
    cats: Vec<CatTarget>,
    enemies: Vec<EnemyTarget>,
    icon: Option<IconTarget>,
    banner: Option<BannerTarget>,
    images: Vec<ImageTarget>,
    assets: Vec<AssetTarget>,
    prose: Vec<ProseTarget>,
    levels: Vec<LevelTarget>,
    animation: Option<AnimTarget>,
    channels: Option<ChannelTarget>,
    offsets: Option<studio::Offsets>,
    ground: Option<GroundTarget>,
    making: Option<MakeTarget>,
    lineup: Option<foundry::domains::sandbox::Cell>,
}

pub(crate) struct MakeTarget {
    add_stage: Option<Making>,
    drop_stage: Option<Making>,
    add_map: Option<Making>,
    drop_map: Option<Making>,
    stages_full: bool,
    maps_full: bool,
    fixed: bool,
}

struct GroundTarget {
    file: String,
    game: PathBuf,
    mod_copy: Option<PathBuf>,
    label: String,
    unlocked: bool,
    active_mod: Option<String>,
}

struct ChannelTarget {
    channels: studio::Channels,
}

struct AnimTarget {
    key: String,
    motion: Option<String>,
    asset: Asset,
    titles: Vec<Option<String>>,
    unlocked: bool,
    active_mod: Option<String>,
}

struct AssetTarget {
    asset: Asset,
    unlocked: bool,
    active_mod: Option<String>,
}

const ACQUISITION_ONLY: [(figures::Subject, &str); 1] =
    [(figures::Subject::Talents, cat_files::SKILL_ACQUISITION)];

fn scene(app: &HaganeApp) -> Page {
    if app.current_page != Page::Sandbox {
        return app.current_page;
    }

    match app.app_state.sandbox.tab {
        SandboxTab::Lineup if app.sandbox_state.inspector().is_some() => Page::Cats,
        SandboxTab::Stage => Page::Stages,
        _ => Page::Sandbox,
    }
}

fn inspecting(app: &HaganeApp) -> Option<&crate::domains::cat::State> {
    let on_lineup = app.current_page == Page::Sandbox && app.app_state.sandbox.tab == SandboxTab::Lineup;

    on_lineup.then(|| app.sandbox_state.inspector()).flatten()
}

fn seat(app: &HaganeApp) -> Seat {
    if app.current_page == Page::Mods {
        return Seat::Mods;
    }

    inspecting(app).map_or(Seat::Main, |_| Seat::Sandbox)
}

fn held_view(app: &HaganeApp) -> &crate::domains::cat::State {
    inspecting(app).unwrap_or(&app.cat_state)
}

fn held_cat(app: &HaganeApp) -> Option<u32> {
    inspecting(app).map_or(app.app_state.cat.selected_cat, |inspector| inspector.selected_cat)
}

fn held_form(app: &HaganeApp) -> usize {
    inspecting(app).map_or(app.app_state.cat.selected_form, |inspector| inspector.selected_form)
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Focus {
    page: Page,
    held: core::Focus,
}

impl Deref for Focus {
    type Target = core::Focus;

    fn deref(&self) -> &core::Focus {
        &self.held
    }
}

pub(crate) fn paged(realm: core::Realm) -> Page {
    match realm {
        core::Realm::Home => Page::Home,
        core::Realm::Cats => Page::Cats,
        core::Realm::Enemies => Page::Enemies,
        core::Realm::Stages => Page::Stages,
        core::Realm::Files => Page::Files,
        core::Realm::Elsewhere => Page::Mods,
    }
}

fn realm(page: Page) -> core::Realm {
    match page {
        Page::Home => core::Realm::Home,
        Page::Cats => core::Realm::Cats,
        Page::Enemies => core::Realm::Enemies,
        Page::Stages => core::Realm::Stages,
        Page::Files => core::Realm::Files,
        _ => core::Realm::Elsewhere,
    }
}

impl Focus {
    fn held(app: &HaganeApp) -> Focus {
        let page = scene(app);

        Focus {
            page,
            held: core::Focus {
                realm: realm(page),
                cat: held_cat(app),
                form: held_form(app),
                enemy: app.app_state.enemy.selected_enemy,
                category: app.stage_state.data.selected_category.clone(),
                map: app.stage_state.data.selected_map.clone(),
                stage: app.stage_state.data.selected_stage.clone(),
                active_mod: app.mods_state.active_mod(),
                unlocked: app.settings.files.unlock_game_mount,
            },
        }
    }
}

struct IconTarget {
    asset: Exception,
    title: &'static str,
    unlocked: bool,
    active_mod: Option<String>,
}

struct ImageTarget {
    asset: Asset,
    title: &'static str,
    unlocked: bool,
    active_mod: Option<String>,
}

struct BannerTarget {
    key: String,
    forms: Vec<(Option<&'static str>, Exception)>,
    unlocked: bool,
    active_mod: Option<String>,
}

impl AnimTarget {
    fn scopes(&self, in_mod: bool) -> Vec<Scope<'_>> {
        self.asset
            .scopes(in_mod)
            .into_iter()
            .zip(&self.titles)
            .map(|(scope, title)| Scope { title: title.as_deref(), ..scope })
            .collect()
    }
}

impl BannerTarget {
    fn scopes(&self, in_mod: bool) -> Vec<Scope<'_>> {
        self.forms
            .iter()
            .flat_map(|(title, form)| {
                form.scopes(in_mod).into_iter().map(|scope| Scope { title: *title, ..scope })
            })
            .collect()
    }
}

const NORMAL_FORM: usize = 0;

struct FileTarget {
    source: PathBuf,
    game: Option<PathBuf>,
    name: String,
    mount: String,
    folder: bool,
    unlocked: bool,
    active_mod: Option<String>,
    mod_copy: Option<PathBuf>,
}

pub(super) type Trail = Vec<usize>;

#[derive(Clone, Debug)]
pub enum Message {
    Opened(Point, Option<Target>),
    Dismissed,
    Invoked(Trail),
    Hovered(Trail),
    ConfirmExpired,
    FailureExpired,
    Replaced(bool),
    Figures(Seat, figures::Subject, figures::Message),
    Ground(Seat, ground::Message),
    Prose(Seat, prose::Subject, prose::Message),
    Find(finder::Message),
    Artwork(artwork::Message),
    Sheet(sheet::Message),
    Ledger(ledger::Message),
    Records(records::Message),
    Preset(preset::Message),
    Group(group::Message),
    Treasure(treasure::Message),
    Gift(gift::Message),
    Filter(filter::Message),
    Evolve(evolve::Message),
    Roster(roster::Message),
    Captions(captions::Message),
    Catfruit(catfruit::Message),
    MapLayout(maplayout::Message),
    Dungeon(dungeon::Message),
    Cuts(cuts::Message),
    Logins(logins::Message),
    Unused(unused::Message),
    Audio(audio::Message),
}

struct Item {
    label: String,
    hint: Option<String>,
    action: Option<Action>,
    children: Vec<Item>,
    confirm: bool,
    terse: bool,
}

impl Item {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self { label: label.into(), hint: None, action: Some(action), children: Vec::new(), confirm: false, terse: false }
    }

    fn disabled(label: impl Into<String>, hint: impl Into<String>) -> Self {
        Self { label: label.into(), hint: Some(hint.into()), action: None, children: Vec::new(), confirm: false, terse: false }
    }

    fn unsupported(label: impl Into<String>) -> Self {
        Self::disabled(label, UNSUPPORTED_NOTICE)
    }

    fn list(label: impl Into<String>, children: Vec<Item>) -> Self {
        let hint = shared_hint(&children);

        Self { label: label.into(), hint, action: None, children, confirm: false, terse: false }
    }

    fn opens(&self) -> bool {
        !self.children.is_empty()
    }

    fn live(&self) -> bool {
        self.action.is_some() || self.children.iter().any(Item::live)
    }

    fn relabel(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    fn confirming(mut self) -> Self {
        self.confirm = true;
        self
    }

    fn confirming_terse(mut self) -> Self {
        self.confirm = true;
        self.terse = true;
        self
    }
}

fn shared_hint(children: &[Item]) -> Option<String> {
    if children.iter().any(Item::live) {
        return None;
    }

    let mut hints = children.iter().map(|child| child.hint.as_deref());
    let first = hints.next().flatten()?;

    hints.all(|hint| hint == Some(first)).then(|| first.to_owned())
}

enum Action {
    Add { source: PathBuf, target_mod: String },
    Delete { source: PathBuf },
    AddChannel { part: usize, kind: i32 },
    DropChannel { track: usize },
    AddPart { parent: Option<usize> },
    AddOffset,
    DropOffset { row: usize },
    Locate { part: usize },
    DropMember { cell: foundry::domains::sandbox::Cell },
    DropPart { part: usize },
    EditAnimation(AnimPlan),
    EditFigures(figures::Plan),
    EditGround(ground::Plan),
    EditProse(prose::Plan),
    Replace { file: String, target_mod: Option<String>, game: Option<PathBuf> },
    Make(Box<Making>),
    Unmake(Box<Making>),
    Sync { file: String, target_mod: String, game: PathBuf },
    Open { source: PathBuf },
    Find { source: PathBuf },
}

#[derive(Clone)]
pub(crate) struct Making {
    target_mod: Option<String>,
    unlocked: bool,
    map: u32,
    stage: u32,
    global: u32,
    whole_map: bool,
    ground: String,
    data_file: String,
    name_file: String,
    plate: String,
    map_plate: String,
    name_key: u32,
    name_cell: usize,
    story: bool,
    vanilla: bool,
    label: String,
}

impl Making {
    fn mount(&self) -> &str {
        self.target_mod.as_deref().unwrap_or(architecture::GAME)
    }

    fn subject(&self) -> String {
        match self.whole_map {
            true => format!("Map {:03}", self.map),
            false => format!("Stage {:02}", self.stage),
        }
    }
}

enum Outcome {
    Done,
    Rescanned,
    Opened(Page),
    Deferred(Task<Message>),
    Failed,
}

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
const PROBE_BYTES: u64 = 1024;

pub(super) enum Format {
    Text,
    Image,
    Binary,
}

pub(super) fn classify(path: &Path) -> Format {
    let Ok(file) = fs::File::open(path) else {
        return Format::Binary;
    };

    let mut probe = Vec::new();

    if file.take(PROBE_BYTES).read_to_end(&mut probe).is_err() {
        return Format::Binary;
    }

    if probe.starts_with(&PNG_MAGIC) {
        return Format::Image;
    }

    if probe.contains(&0) {
        return Format::Binary;
    }

    match std::str::from_utf8(&probe) {
        Ok(_) => Format::Text,
        Err(err) if err.error_len().is_none() => Format::Text,
        Err(_) => Format::Binary,
    }
}

fn pick<'a>(items: &'a [Item], trail: &[usize]) -> Option<&'a Item> {
    let (last, parents) = trail.split_last()?;
    let mut level = items;

    for index in parents {
        level = &level.get(*index)?.children;
    }

    level.get(*last)
}

fn panels<'a>(items: &'a [Item], trail: &[usize]) -> Vec<&'a [Item]> {
    let mut open = Vec::with_capacity(trail.len());
    let mut level = items;

    for index in trail {
        let Some(item) = level.get(*index).filter(|item| item.opens()) else {
            break;
        };

        level = item.children.as_slice();
        open.push(level);
    }

    open
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seat {
    Main,
    Sandbox,
    Mods,
}

const SEATS: usize = 3;

impl Seat {
    const ALL: [Seat; SEATS] = [Seat::Main, Seat::Sandbox, Seat::Mods];

    fn slot(self) -> usize {
        match self {
            Seat::Main => 0,
            Seat::Sandbox => 1,
            Seat::Mods => 2,
        }
    }

    fn owns(self, held: Page, page: Page) -> bool {
        self == Seat::Mods || held == page
    }
}

struct Pins {
    figures: [Option<Pick>; figures::COUNT],
    prose: [Option<Pick>; prose::COUNT],
    ground: Option<Pick>,
    artwork: Option<Pick>,
    sheet: Option<Pick>,
    ledger: Option<Pick>,
    records: Option<Pick>,
    preset: Option<Pick>,
    group: Option<Pick>,
    treasure: Option<Pick>,
    gift: Option<Pick>,
    filter: Option<Pick>,
    evolve: Option<Pick>,
    roster: Option<Pick>,
    captions: Option<Pick>,
    catfruit: Option<Pick>,
    maplayout: Option<Pick>,
    dungeon: Option<Pick>,
    cuts: Option<Pick>,
    logins: Option<Pick>,
    unused: Option<Pick>,
    audio: Option<Pick>,
}

struct Bank {
    figures: [figures::State; figures::COUNT],
    prose: [prose::State; prose::COUNT],
    ground: ground::State,
    artwork: artwork::State,
    sheet: sheet::State,
    ledger: ledger::State,
    records: records::State,
    preset: preset::State,
    group: group::State,
    treasure: treasure::State,
    gift: gift::State,
    filter: filter::State,
    evolve: evolve::State,
    roster: roster::State,
    captions: captions::State,
    catfruit: catfruit::State,
    maplayout: maplayout::State,
    dungeon: dungeon::State,
    cuts: cuts::State,
    logins: logins::State,
    unused: unused::State,
    audio: audio::State,
    glossary_aim: Option<(Identity, Faction)>,
    pins: Pins,
}

impl Default for Pins {
    fn default() -> Self {
        Pins {
            figures: std::array::from_fn(|_| None),
            prose: Default::default(),
            ground: None,
            artwork: None,
            sheet: None,
            ledger: None,
            records: None,
            preset: None,
            group: None,
            treasure: None,
            gift: None,
            filter: None,
            evolve: None,
            roster: None,
            captions: None,
            catfruit: None,
            maplayout: None,
            dungeon: None,
            cuts: None,
            logins: None,
            unused: None,
            audio: None,
        }
    }
}

impl Default for Bank {
    fn default() -> Self {
        Bank {
            figures: std::array::from_fn(|_| figures::State::default()),
            prose: Default::default(),
            ground: ground::State::default(),
            artwork: artwork::State::default(),
            sheet: sheet::State::default(),
            ledger: ledger::State::default(),
            records: records::State::default(),
            preset: preset::State::default(),
            group: group::State::default(),
            treasure: treasure::State::default(),
            gift: gift::State::default(),
            filter: filter::State::default(),
            evolve: evolve::State::default(),
            roster: roster::State::default(),
            captions: captions::State::default(),
            catfruit: catfruit::State::default(),
            maplayout: maplayout::State::default(),
            dungeon: dungeon::State::default(),
            cuts: cuts::State::default(),
            logins: logins::State::default(),
            unused: unused::State::default(),
            audio: audio::State::default(),
            glossary_aim: None,
            pins: Pins::default(),
        }
    }
}

impl Bank {
    fn unpin(&mut self) {
        for subject in figures::SUBJECTS {
            if !self.figures[subject.slot()].drafting() {
                self.pins.figures[subject.slot()] = None;
            }
        }

        for subject in prose::SUBJECTS {
            if !self.prose[subject.slot()].drafting() {
                self.pins.prose[subject.slot()] = None;
            }
        }

        if !self.ground.drafting() {
            self.pins.ground = None;
        }

        if !self.artwork.drafting() {
            self.pins.artwork = None;
        }

        if !self.sheet.drafting() {
            self.pins.sheet = None;
        }

        if !self.ledger.drafting() {
            self.pins.ledger = None;
        }

        if !self.records.drafting() {
            self.pins.records = None;
        }

        if !self.preset.drafting() {
            self.pins.preset = None;
        }

        if !self.group.drafting() {
            self.pins.group = None;
        }

        if !self.treasure.drafting() {
            self.pins.treasure = None;
        }

        if !self.gift.drafting() {
            self.pins.gift = None;
        }

        if !self.filter.drafting() {
            self.pins.filter = None;
        }

        if !self.evolve.drafting() {
            self.pins.evolve = None;
        }

        if !self.roster.drafting() {
            self.pins.roster = None;
        }

        if !self.captions.drafting() {
            self.pins.captions = None;
        }

        if !self.catfruit.drafting() {
            self.pins.catfruit = None;
        }

        if !self.maplayout.drafting() {
            self.pins.maplayout = None;
        }

        if !self.dungeon.drafting() {
            self.pins.dungeon = None;
        }

        if !self.cuts.drafting() {
            self.pins.cuts = None;
        }

        if !self.logins.drafting() {
            self.pins.logins = None;
        }

        if !self.unused.drafting() {
            self.pins.unused = None;
        }

        if !self.audio.drafting() {
            self.pins.audio = None;
        }
    }

    fn drafting(&self) -> bool {
        self.figures.iter().any(figures::State::drafting)
            || self.prose.iter().any(prose::State::drafting)
            || self.ground.drafting()
            || self.artwork.drafting()
            || self.sheet.drafting()
            || self.ledger.drafting()
            || self.records.drafting()
            || self.preset.drafting()
            || self.group.drafting()
            || self.treasure.drafting()
            || self.gift.drafting()
            || self.filter.drafting()
            || self.evolve.drafting()
            || self.roster.drafting()
            || self.captions.drafting()
            || self.catfruit.drafting()
            || self.maplayout.drafting()
            || self.dungeon.drafting()
            || self.cuts.drafting()
            || self.logins.drafting()
            || self.unused.drafting()
            || self.audio.drafting()
    }

    fn drifted(&self) -> bool {
        self.figures.iter().any(figures::State::drifted)
            || self.prose.iter().any(prose::State::drifted)
            || self.ground.drifted()
            || self.artwork.drifted()
            || self.sheet.drifted()
            || self.ledger.drifted()
            || self.records.drifted()
            || self.preset.drifted()
            || self.group.drifted()
            || self.treasure.drifted()
            || self.gift.drifted()
            || self.filter.drifted()
            || self.evolve.drifted()
            || self.roster.drifted()
            || self.captions.drifted()
            || self.catfruit.drifted()
            || self.maplayout.drifted()
            || self.dungeon.drifted()
            || self.cuts.drifted()
            || self.logins.drifted()
            || self.audio.drifted()
    }

    fn stacked(&self, seat: Seat, page: Page) -> usize {
        let prose = prose::SUBJECTS
            .into_iter()
            .filter(|subject| seat.owns(subject.page(), page) && self.prose[subject.slot()].drafting())
            .count();

        let figures = figures::SUBJECTS
            .into_iter()
            .filter(|subject| seat.owns(subject.page(), page) && self.figures[subject.slot()].drafting())
            .count();

        let pinned = |drafting: bool| usize::from(seat == Seat::Mods && drafting);

        prose
            + figures
            + pinned(self.ground.drafting())
            + pinned(self.artwork.drafting())
            + pinned(self.sheet.drafting())
            + pinned(self.ledger.drafting())
            + pinned(self.records.drafting())
            + pinned(self.preset.drafting())
            + pinned(self.group.drafting())
            + pinned(self.treasure.drafting())
            + pinned(self.gift.drafting())
            + pinned(self.filter.drafting())
            + pinned(self.evolve.drafting())
            + pinned(self.roster.drafting())
            + pinned(self.captions.drafting())
            + pinned(self.catfruit.drafting())
            + pinned(self.maplayout.drafting())
            + pinned(self.dungeon.drafting())
            + pinned(self.cuts.drafting())
            + pinned(self.logins.drafting())
            + pinned(self.unused.drafting())
            + pinned(self.audio.drafting())
    }

    fn relocalize(&self) {
        for slot in &self.figures {
            slot.relocalize();
        }

        self.ground.clear_icons();
    }

    fn flush_now(&mut self, vfs: &Vfs) {
        for slot in &mut self.figures {
            slot.flush_now(vfs);
        }

        for slot in &mut self.prose {
            slot.flush_now(vfs);
        }

        self.ground.flush_now(vfs);
    }

    fn release(&mut self, keep: Option<&str>, vfs: &Vfs) {
        let kept = |pin: &Option<Pick>| pin.as_ref().is_some_and(|pick| keep.is_some_and(|name| pick.focus.mount() == Some(name)));

        for subject in figures::SUBJECTS {
            let slot = subject.slot();

            if kept(&self.pins.figures[slot]) {
                continue;
            }

            self.figures[slot].flush_now(vfs);
            self.figures[slot].sync(None, vfs);
            self.pins.figures[slot] = None;
        }

        for subject in prose::SUBJECTS {
            let slot = subject.slot();

            if kept(&self.pins.prose[slot]) {
                continue;
            }

            self.prose[slot].flush_now(vfs);
            self.prose[slot].sync(&[], vfs);
            self.pins.prose[slot] = None;
        }

        if !kept(&self.pins.ground) {
            self.ground.flush_now(vfs);
            self.ground.sync(None, vfs);
            self.pins.ground = None;
        }

        if !kept(&self.pins.artwork) {
            self.artwork.sync(None, vfs);
            self.pins.artwork = None;
        }

        if !kept(&self.pins.sheet) {
            self.sheet.sync(None, vfs);
            self.pins.sheet = None;
        }

        if !kept(&self.pins.ledger) {
            self.ledger.sync(None, vfs);
            self.pins.ledger = None;
        }

        if !kept(&self.pins.records) {
            self.records.sync(None, vfs);
            self.pins.records = None;
        }

        if !kept(&self.pins.preset) {
            self.preset.sync(None, vfs);
            self.pins.preset = None;
        }

        if !kept(&self.pins.group) {
            self.group.sync(None, vfs);
            self.pins.group = None;
        }

        if !kept(&self.pins.treasure) {
            self.treasure.sync(None, vfs);
            self.pins.treasure = None;
        }

        if !kept(&self.pins.gift) {
            self.gift.sync(None, vfs);
            self.pins.gift = None;
        }

        if !kept(&self.pins.filter) {
            self.filter.sync(None, vfs);
            self.pins.filter = None;
        }

        if !kept(&self.pins.evolve) {
            self.evolve.sync(None, vfs);
            self.pins.evolve = None;
        }

        if !kept(&self.pins.roster) {
            self.roster.sync(None, vfs);
            self.pins.roster = None;
        }

        if !kept(&self.pins.captions) {
            self.captions.sync(None, vfs);
            self.pins.captions = None;
        }

        if !kept(&self.pins.catfruit) {
            self.catfruit.sync(None, vfs);
            self.pins.catfruit = None;
        }

        if !kept(&self.pins.maplayout) {
            self.maplayout.sync(None, vfs);
            self.pins.maplayout = None;
        }

        if !kept(&self.pins.dungeon) {
            self.dungeon.sync(None, vfs);
            self.pins.dungeon = None;
        }

        if !kept(&self.pins.cuts) {
            self.cuts.sync(None, vfs);
            self.pins.cuts = None;
        }

        if !kept(&self.pins.logins) {
            self.logins.sync(None, vfs);
            self.pins.logins = None;
        }

        if !kept(&self.pins.unused) {
            self.unused.sync(None);
            self.pins.unused = None;
        }

        if !kept(&self.pins.audio) {
            self.audio.sync(None, vfs);
            self.pins.audio = None;
        }
    }

    fn flush(&mut self, seat: Seat, vfs: &Vfs) -> Vec<Task<Message>> {
        let figures = figures::SUBJECTS
            .into_iter()
            .map(|subject| self.figures[subject.slot()].flush(vfs).map(move |inner| Message::Figures(seat, subject, inner)));

        let prose = prose::SUBJECTS
            .into_iter()
            .map(|subject| self.prose[subject.slot()].flush(vfs).map(move |inner| Message::Prose(seat, subject, inner)));

        figures.chain(prose).collect()
    }
}

#[derive(Default)]
pub(crate) struct State {
    open: Option<Open>,
    opened: Option<Page>,
    rescan: bool,
    dropped: Option<foundry::domains::sandbox::Cell>,
    pending: Option<Trail>,
    confirm: Slot<Trail>,
    failed: Slot<Trail>,
    banks: [Bank; SEATS],
    finder: finder::State,
    synced: Option<Key>,
}

struct Snapshot {
    page: Page,
    seat: Seat,
    figures: [Option<figures::Plan>; figures::COUNT],
    prose: [Vec<prose::Plan>; prose::COUNT],
    ground: Option<ground::Plan>,
    artwork: Option<artwork::Plan>,
    sheet: Option<sheet::Plan>,
    ledger: Option<ledger::Plan>,
    records: Option<records::Plan>,
    preset: Option<preset::Plan>,
    group: Option<group::Plan>,
    treasure: Option<treasure::Plan>,
    gift: Option<gift::Plan>,
    filter: Option<filter::Plan>,
    evolve: Option<evolve::Plan>,
    roster: Option<roster::Plan>,
    captions: Option<captions::Plan>,
    catfruit: Option<catfruit::Plan>,
    maplayout: Option<maplayout::Plan>,
    dungeon: Option<dungeon::Plan>,
    cuts: Option<cuts::Plan>,
    logins: Option<logins::Plan>,
    unused: Option<unused::Plan>,
    audio: Option<audio::Plan>,
}

#[derive(PartialEq)]
struct Key {
    seat: Seat,
    values: EditorMode,
    focus: Focus,
}

pub(crate) struct Update {
    snapshot: Snapshot,
    key: Key,
}

struct Open {
    at: Point,
    items: Vec<Item>,
    trail: Trail,
    seat: Seat,
}

impl State {
    pub(crate) fn open(&mut self, at: Point, context: &Context) {
        let mut items = registry::items(context);

        if items.is_empty() {
            trace!(
                page = ?context.page,
                targeted = context.file.is_some(),
                "Right click produced no context menu actions"
            );

            items.push(Item::disabled(NO_ACTIONS_LABEL, NO_ACTIONS_NOTICE));
        }

        self.confirm.expire();
        self.failed.expire();
        self.pending = None;
        self.open = (!items.is_empty()).then_some(Open { at, items, trail: Trail::new(), seat: context.seat });
    }

    pub(crate) fn update(&mut self, message: Message, vfs: &Vfs, studio: &mut studio::State) -> Task<Message> {
        match message {
            Message::Invoked(trail) => self.invoke(trail, vfs, studio),
            Message::Hovered(trail) => {
                if let Some(open) = self.open.as_mut() {
                    open.trail = trail;
                }

                Task::none()
            }
            Message::Dismissed => {
                self.open = None;
                self.pending = None;
                self.confirm.expire();
                self.failed.expire();
                Task::none()
            }
            Message::ConfirmExpired => {
                self.confirm.expire();
                Task::none()
            }
            Message::FailureExpired => {
                self.failed.expire();
                Task::none()
            }
            Message::Replaced(placed) => {
                let Some(trail) = self.pending.take() else { return Task::none(); };

                if placed {
                    self.open = None;
                    return Task::none();
                }

                self.failed.set(trail, Message::FailureExpired)
            }
            Message::Prose(seat, subject, msg) => {
                let bank = &mut self.banks[seat.slot()];
                let task = bank.prose[subject.slot()].update(msg, vfs).map(move |inner| Message::Prose(seat, subject, inner));

                bank.unpin();

                task
            }
            Message::Figures(seat, subject, msg) => {
                let bank = &mut self.banks[seat.slot()];
                let task = bank.figures[subject.slot()].update(msg, vfs).map(move |inner| Message::Figures(seat, subject, inner));

                bank.unpin();

                task
            }
            Message::Ground(seat, msg) => {
                let bank = &mut self.banks[seat.slot()];
                let task = bank.ground.update(msg, vfs).map(move |inner| Message::Ground(seat, inner));

                bank.unpin();

                task
            }
            Message::Find(msg) => self.finder.update(msg).map(Message::Find),
            Message::Artwork(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.artwork.update(msg, vfs).map(Message::Artwork);

                bank.unpin();

                task
            }
            Message::Sheet(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.sheet.update(msg, vfs).map(Message::Sheet);

                bank.unpin();

                task
            }
            Message::Ledger(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.ledger.update(msg, vfs).map(Message::Ledger);

                bank.unpin();

                task
            }
            Message::Records(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.records.update(msg, vfs).map(Message::Records);

                bank.unpin();

                task
            }
            Message::Preset(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.preset.update(msg, vfs).map(Message::Preset);

                bank.unpin();

                task
            }
            Message::Group(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.group.update(msg, vfs).map(Message::Group);

                bank.unpin();

                task
            }
            Message::Treasure(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.treasure.update(msg, vfs).map(Message::Treasure);

                bank.unpin();

                task
            }
            Message::Gift(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.gift.update(msg, vfs).map(Message::Gift);

                bank.unpin();

                task
            }
            Message::Filter(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.filter.update(msg, vfs).map(Message::Filter);

                bank.unpin();

                task
            }
            Message::Evolve(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.evolve.update(msg, vfs).map(Message::Evolve);

                bank.unpin();

                task
            }
            Message::Roster(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.roster.update(msg, vfs).map(Message::Roster);

                bank.unpin();

                task
            }
            Message::Captions(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.captions.update(msg, vfs).map(Message::Captions);

                bank.unpin();

                task
            }
            Message::Catfruit(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.catfruit.update(msg, vfs).map(Message::Catfruit);

                bank.unpin();

                task
            }
            Message::MapLayout(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.maplayout.update(msg, vfs).map(Message::MapLayout);

                bank.unpin();

                task
            }
            Message::Dungeon(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.dungeon.update(msg, vfs).map(Message::Dungeon);

                bank.unpin();

                task
            }
            Message::Cuts(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.cuts.update(msg, vfs).map(Message::Cuts);

                bank.unpin();

                task
            }
            Message::Logins(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.logins.update(msg, vfs).map(Message::Logins);

                bank.unpin();

                task
            }
            Message::Unused(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.unused.update(msg).map(Message::Unused);

                bank.unpin();

                task
            }
            Message::Audio(msg) => {
                let bank = &mut self.banks[Seat::Mods.slot()];
                let task = bank.audio.update(msg, vfs).map(Message::Audio);

                bank.unpin();

                task
            }
            Message::Opened(..) => Task::none(),
        }
    }

    pub(crate) fn take_opened(&mut self) -> Option<Page> {
        self.opened.take()
    }

    pub(crate) fn take_dropped(&mut self) -> Option<foundry::domains::sandbox::Cell> {
        self.dropped.take()
    }

    pub(crate) fn take_rescan(&mut self) -> bool {
        std::mem::take(&mut self.rescan)
    }

    pub(crate) fn popup_view<'a>(
        &'a self,
        app: &'a HaganeApp,
        window: Size,
    ) -> Vec<(u64, popup::Kind, Element<'a, Message>)> {
        let mut views: Vec<(u64, popup::Kind, Element<'_, Message>)> = Vec::new();
        let seat = seat(app);
        let bank = &self.banks[seat.slot()];
        let pinned = seat == Seat::Mods;
        let held = held_cat(app);
        let (held_cap, held_used) = (level_cap(app, held), talent_costs(&sources(app), held));

        for subject in prose::SUBJECTS {
            if !pinned && (subject.page() != scene(app) || !prose_tab(app, subject)) {
                continue;
            }

            let slot = &bank.prose[subject.slot()];

            if let Some(view) = slot.view(window) {
                views.push((slot.raised(), subject.kind(), view.map(move |inner| Message::Prose(seat, subject, inner))));
            }
        }

        for subject in figures::SUBJECTS {
            if !pinned && (subject.page() != scene(app) || !figures_tab(app, subject)) {
                continue;
            }

            let slot = &bank.figures[subject.slot()];

            if !slot.drafting() {
                continue;
            }

            let (cap, used) = match pinned {
                true => {
                    let cat = bank.pins.figures[subject.slot()].as_ref().and_then(|pick| pick.focus.cat);

                    (level_cap(app, cat), talent_costs(&sources(app), cat))
                }
                false => (held_cap, held_used),
            };

            if let Some(view) = slot.view(window, cap, used, &app.cat_state.data.cats, &app.vault, &app.names) {
                views.push((slot.raised(), figures::kind(subject), view.map(move |inner| Message::Figures(seat, subject, inner))));
            }
        }

        if pinned || scene(app) == Page::Stages {
            let enemies = &app.stage_state.data.enemy_registry;

            if let Some(view) = bank.ground.view(window, enemies) {
                views.push((bank.ground.raised(), ground::kind(), view.map(move |inner| Message::Ground(seat, inner))));
            }

            if let Some(view) = bank.ground.picker_view(window, enemies) {
                views.push((bank.ground.raised() + 1, ground::pick_kind(), view.map(move |inner| Message::Ground(seat, inner))));
            }
        }

        if pinned && let Some(view) = bank.artwork.view(window) {
            views.push((bank.artwork.raised(), artwork::kind(), view.map(Message::Artwork)));
        }

        if pinned && let Some(view) = bank.sheet.view(window) {
            views.push((bank.sheet.raised(), sheet::kind(), view.map(Message::Sheet)));
        }

        if pinned && let Some(view) = bank.ledger.view(window) {
            views.push((bank.ledger.raised(), ledger::kind(), view.map(Message::Ledger)));
        }

        if pinned && let Some(view) = bank.records.view(window) {
            views.push((bank.records.raised(), records::kind(), view.map(Message::Records)));
        }

        if pinned && let Some(view) = bank.preset.view(window) {
            views.push((bank.preset.raised(), preset::kind(), view.map(Message::Preset)));
        }

        if pinned && let Some(view) = bank.group.view(window) {
            views.push((bank.group.raised(), group::kind(), view.map(Message::Group)));
        }

        if pinned && let Some(view) = bank.treasure.view(window) {
            views.push((bank.treasure.raised(), treasure::kind(), view.map(Message::Treasure)));
        }

        if pinned && let Some(view) = bank.gift.view(window) {
            views.push((bank.gift.raised(), gift::kind(), view.map(Message::Gift)));
        }

        if pinned && let Some(view) = bank.filter.view(window) {
            views.push((bank.filter.raised(), filter::kind(), view.map(Message::Filter)));
        }

        if pinned && let Some(view) = bank.evolve.view(window) {
            views.push((bank.evolve.raised(), evolve::kind(), view.map(Message::Evolve)));
        }

        if pinned && let Some(view) = bank.roster.view(window) {
            views.push((bank.roster.raised(), roster::kind(), view.map(Message::Roster)));
        }

        if pinned && let Some(view) = bank.captions.view(window) {
            views.push((bank.captions.raised(), captions::kind(), view.map(Message::Captions)));
        }

        if pinned && let Some(view) = bank.catfruit.view(window) {
            views.push((bank.catfruit.raised(), catfruit::kind(), view.map(Message::Catfruit)));
        }

        if pinned && let Some(view) = bank.maplayout.view(window) {
            views.push((bank.maplayout.raised(), maplayout::kind(), view.map(Message::MapLayout)));
        }

        if pinned && let Some(view) = bank.dungeon.view(window) {
            views.push((bank.dungeon.raised(), dungeon::kind(), view.map(Message::Dungeon)));
        }

        if pinned && let Some(view) = bank.cuts.view(window) {
            views.push((bank.cuts.raised(), cuts::kind(), view.map(Message::Cuts)));
        }

        if pinned && let Some(view) = bank.logins.view(window) {
            views.push((bank.logins.raised(), logins::kind(), view.map(Message::Logins)));
        }

        if pinned && let Some(view) = bank.unused.view(window) {
            views.push((bank.unused.raised(), unused::kind(), view.map(Message::Unused)));
        }

        if pinned && let Some(view) = bank.audio.view(window) {
            views.push((bank.audio.raised(), audio::kind(), view.map(Message::Audio)));
        }

        if pinned && let Some(view) = self.finder.view(window, app.settings.files.editor_mode) {
            views.push((self.finder.raised(), finder::kind(), view.map(Message::Find)));
        }

        views
    }

    #[cfg(test)]
    pub(crate) fn kinds() -> Vec<popup::Kind> {
        prose::SUBJECTS
            .into_iter()
            .map(prose::Subject::kind)
            .chain(figures::SUBJECTS.into_iter().map(figures::kind))
            .chain([ground::kind(), ground::pick_kind(), finder::kind(), artwork::kind(), sheet::kind(), ledger::kind(), records::kind(), preset::kind(), group::kind(), treasure::kind(), gift::kind(), filter::kind(), evolve::kind(), roster::kind(), captions::kind(), catfruit::kind(), maplayout::kind(), dungeon::kind(), unused::kind(), audio::kind(), cuts::kind(), logins::kind()])
            .collect()
    }

    pub(crate) fn restore_scroll<M: Send + 'static>(&self, app: &HaganeApp) -> Task<M> {
        let bank = &self.banks[seat(app).slot()];
        let tasks: Vec<Task<M>> =
            figures::SUBJECTS.iter().filter_map(|subject| bank.figures[subject.slot()].restore_scroll()).collect();

        Task::batch(tasks)
    }

    pub(crate) fn relocalize(&self) {
        for bank in &self.banks {
            bank.relocalize();
        }
    }

    pub(crate) fn forget_enemies<'a>(&self, ids: impl IntoIterator<Item = &'a u32>) {
        for id in ids {
            for bank in &self.banks {
                bank.ground.forget(*id);
            }
        }
    }

    pub(crate) fn flush_now(&mut self, vfs: &Vfs) {
        for bank in &mut self.banks {
            bank.flush_now(vfs);
        }
    }

    pub(crate) fn flush_drafts(&mut self, vfs: &Vfs) -> Task<Message> {
        let tasks: Vec<Task<Message>> = Seat::ALL.into_iter().flat_map(|seat| self.banks[seat.slot()].flush(seat, vfs)).collect();

        Task::batch(tasks)
    }

    fn drafting(&self) -> bool {
        self.banks.iter().any(Bank::drafting)
    }

    fn figures_drafting(&self, seat: Seat, subject: figures::Subject) -> bool {
        self.banks[seat.slot()].figures[subject.slot()].drafting()
    }

    fn prose_drafting(&self, seat: Seat, subject: prose::Subject) -> bool {
        self.banks[seat.slot()].prose[subject.slot()].drafting()
    }

    fn glossary_aim(&self, seat: Seat) -> Option<(Identity, Faction)> {
        self.banks[seat.slot()].glossary_aim
    }

    fn stale(&self, key: &Key) -> bool {
        self.synced.as_ref() != Some(key) || self.drifted(key.seat)
    }

    fn drifted(&self, seat: Seat) -> bool {
        self.banks[seat.slot()].drifted()
    }

    pub(crate) fn apply(&mut self, update: Update, vfs: &Vfs) {
        let Update { snapshot, key } = update;
        self.synced = Some(key);
        let seat = snapshot.seat;
        let bank = &mut self.banks[seat.slot()];

        for subject in prose::SUBJECTS {
            if !seat.owns(subject.page(), snapshot.page) {
                continue;
            }

            bank.prose[subject.slot()].sync(&snapshot.prose[subject.slot()], vfs);
        }

        bank.ground.sync(snapshot.ground, vfs);
        bank.artwork.sync(snapshot.artwork, vfs);
        bank.sheet.sync(snapshot.sheet, vfs);
        bank.ledger.sync(snapshot.ledger, vfs);
        bank.records.sync(snapshot.records, vfs);
        bank.preset.sync(snapshot.preset, vfs);
        bank.group.sync(snapshot.group, vfs);
        bank.treasure.sync(snapshot.treasure, vfs);
        bank.gift.sync(snapshot.gift, vfs);
        bank.filter.sync(snapshot.filter, vfs);
        bank.evolve.sync(snapshot.evolve, vfs);
        bank.roster.sync(snapshot.roster, vfs);
        bank.captions.sync(snapshot.captions, vfs);
        bank.catfruit.sync(snapshot.catfruit, vfs);
        bank.maplayout.sync(snapshot.maplayout, vfs);
        bank.dungeon.sync(snapshot.dungeon, vfs);
        bank.cuts.sync(snapshot.cuts, vfs);
        bank.logins.sync(snapshot.logins, vfs);
        bank.unused.sync(snapshot.unused);
        bank.audio.sync(snapshot.audio, vfs);

        for (slot, plan) in snapshot.figures.into_iter().enumerate() {
            if !seat.owns(figures::SUBJECTS[slot].page(), snapshot.page) {
                continue;
            }

            bank.figures[slot].sync(plan, vfs);
        }

        if seat == Seat::Mods {
            bank.unpin();
        }
    }

    pub(crate) fn finding(&self) -> bool {
        self.finder.finding()
    }

    pub(crate) fn choosing(&self) -> bool {
        self.finder.choosing()
    }

    pub(crate) fn find(&mut self, catalog: Option<finder::Catalog>) {
        self.finder.open(catalog);
    }

    pub(crate) fn refind(&mut self, catalog: finder::Catalog) {
        self.finder.rebuild(catalog);
    }

    pub(crate) fn take_choice(&mut self, target: Option<String>) -> Option<Pick> {
        self.finder.take_choice(target)
    }

    pub(crate) fn choice_failed(&mut self) -> Task<Message> {
        self.finder.failed().map(Message::Find)
    }

    pub(crate) fn resetting(&self) -> bool {
        self.finder.resetting()
    }

    pub(crate) fn take_reset(&mut self, target: Option<String>) -> Option<Pick> {
        self.finder.take_reset(target)
    }

    pub(crate) fn reset_pick(&mut self, planned: Planned, vfs: &Vfs) -> bool {
        planned.reset(vfs)
    }

    pub(crate) fn reset_failed(&mut self) -> Task<Message> {
        self.finder.reset_failed().map(Message::Find)
    }

    pub(crate) fn unmarked(&self) -> Vec<u32> {
        self.finder.unmarked()
    }

    pub(crate) fn marked(&mut self, marks: Vec<(u32, Mark)>) {
        self.finder.marked(marks);
    }

    pub(crate) fn unmark(&mut self) {
        self.finder.unmark();
    }

    pub(crate) fn begin_pick(&mut self, pick: Pick, planned: Planned, vfs: &Vfs) -> bool {
        let bank = &mut self.banks[Seat::Mods.slot()];

        let (drafting, pin) = match planned {
            Planned::Figures(plan) => {
                let slot = plan.subject().slot();
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.figures[slot].drafting());

                bank.figures[slot].flush_now(vfs);
                bank.figures[slot].begin(plan, nudge, vfs);

                (bank.figures[slot].drafting(), &mut bank.pins.figures[slot])
            }
            Planned::Prose(plan) => {
                let slot = plan.subject().slot();
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.prose[slot].drafting());

                bank.prose[slot].flush_now(vfs);
                bank.prose[slot].begin(plan, nudge, vfs);

                (bank.prose[slot].drafting(), &mut bank.pins.prose[slot])
            }
            Planned::Ground(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.ground.drafting());

                bank.ground.flush_now(vfs);
                bank.ground.begin(plan, nudge, vfs);

                (bank.ground.drafting(), &mut bank.pins.ground)
            }
            Planned::Artwork(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.artwork.drafting());

                bank.artwork.begin(plan, nudge, vfs);

                (bank.artwork.drafting(), &mut bank.pins.artwork)
            }
            Planned::Sheet(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.sheet.drafting());

                bank.sheet.begin(plan, nudge, vfs);

                (bank.sheet.drafting(), &mut bank.pins.sheet)
            }
            Planned::Ledger(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.ledger.drafting());

                bank.ledger.begin(plan, nudge, vfs);

                (bank.ledger.drafting(), &mut bank.pins.ledger)
            }
            Planned::Records(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.records.drafting());

                bank.records.begin(plan, nudge, vfs);

                (bank.records.drafting(), &mut bank.pins.records)
            }
            Planned::Preset(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.preset.drafting());

                bank.preset.begin(plan, nudge, vfs);

                (bank.preset.drafting(), &mut bank.pins.preset)
            }
            Planned::Group(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.group.drafting());

                bank.group.begin(plan, nudge, vfs);

                (bank.group.drafting(), &mut bank.pins.group)
            }
            Planned::Treasure(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.treasure.drafting());

                bank.treasure.begin(plan, nudge, vfs);

                (bank.treasure.drafting(), &mut bank.pins.treasure)
            }
            Planned::Gift(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.gift.drafting());

                bank.gift.begin(plan, nudge, vfs);

                (bank.gift.drafting(), &mut bank.pins.gift)
            }
            Planned::Filter(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.filter.drafting());

                bank.filter.begin(plan, nudge, vfs);

                (bank.filter.drafting(), &mut bank.pins.filter)
            }
            Planned::Evolve(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.evolve.drafting());

                bank.evolve.begin(plan, nudge, vfs);

                (bank.evolve.drafting(), &mut bank.pins.evolve)
            }
            Planned::Roster(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.roster.drafting());

                bank.roster.begin(plan, nudge, vfs);

                (bank.roster.drafting(), &mut bank.pins.roster)
            }
            Planned::Captions(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.captions.drafting());

                bank.captions.begin(plan, nudge, vfs);

                (bank.captions.drafting(), &mut bank.pins.captions)
            }
            Planned::Catfruit(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.catfruit.drafting());

                bank.catfruit.begin(plan, nudge, vfs);

                (bank.catfruit.drafting(), &mut bank.pins.catfruit)
            }
            Planned::MapLayout(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.maplayout.drafting());

                bank.maplayout.begin(plan, nudge, vfs);

                (bank.maplayout.drafting(), &mut bank.pins.maplayout)
            }
            Planned::Dungeon(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.dungeon.drafting());

                bank.dungeon.begin(plan, nudge, vfs);

                (bank.dungeon.drafting(), &mut bank.pins.dungeon)
            }
            Planned::Cuts(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.cuts.drafting());

                bank.cuts.begin(plan, nudge, vfs);

                (bank.cuts.drafting(), &mut bank.pins.cuts)
            }
            Planned::Logins(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.logins.drafting());

                bank.logins.begin(plan, nudge, vfs);

                (bank.logins.drafting(), &mut bank.pins.logins)
            }
            Planned::Unused(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.unused.drafting());

                bank.unused.begin(plan, nudge);

                (bank.unused.drafting(), &mut bank.pins.unused)
            }
            Planned::Audio(plan) => {
                let nudge = bank.stacked(Seat::Mods, Page::Mods) - usize::from(bank.audio.drafting());

                bank.audio.begin(plan, nudge, vfs);

                (bank.audio.drafting(), &mut bank.pins.audio)
            }
        };

        *pin = drafting.then_some(pick);

        drafting
    }

    pub(crate) fn flush_mod(&mut self, vfs: &Vfs) {
        self.banks[Seat::Mods.slot()].flush_now(vfs);
    }

    pub(crate) fn follow_mod(&mut self, selected: Option<&str>, vfs: &Vfs) {
        self.banks[Seat::Mods.slot()].release(selected, vfs);

        if selected.is_none() {
            self.finder.close();
        }
    }

    fn perform(&mut self, seat: Seat, action: &Action, vfs: &Vfs, studio: &mut studio::State) -> Outcome {
        match action {
            Action::Add { source, target_mod } => match mods::adopt(target_mod, source) {
                Ok(path) => {
                    info!(path = %path.display(), "Added a file to a mod");

                    Outcome::Done
                }
                Err(err) => {
                    warn!(source = %source.display(), "Failed to add the file to the mod: {}", err);

                    Outcome::Failed
                }
            },
            Action::Delete { source } => match fs::remove_file(source) {
                Ok(()) => {
                    info!(path = %source.display(), "Deleted a mod file");

                    Outcome::Done
                }
                Err(err) => {
                    warn!(path = %source.display(), "Failed to delete the file: {}", err);

                    Outcome::Failed
                }
            },
            Action::Open { source } => match open::that(source) {
                Ok(()) => {
                    info!(path = %source.display(), "Opened a file in an external program");

                    Outcome::Done
                }
                Err(err) => {
                    warn!(path = %source.display(), "Failed to open the file: {}", err);

                    Outcome::Failed
                }
            },
            Action::Find { source } => {
                let Some(folder) = source.parent() else {
                    warn!(path = %source.display(), "The file has no containing folder to open");

                    return Outcome::Failed;
                };

                match open::that(folder) {
                    Ok(()) => {
                        info!(path = %folder.display(), "Opened a containing folder");

                        Outcome::Done
                    }
                    Err(err) => {
                        warn!(path = %folder.display(), "Failed to open the containing folder: {}", err);

                        Outcome::Failed
                    }
                }
            }
            Action::AddChannel { part, kind } => match studio.add_channel(*part, *kind) {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::DropChannel { track } => match studio.drop_channel(*track) {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::AddPart { parent } => match studio.add_part(*parent) {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::AddOffset => match studio.add_offset() {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::DropOffset { row } => match studio.drop_offset(*row) {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::DropMember { cell } => {
                self.dropped = Some(*cell);

                Outcome::Done
            }
            Action::Locate { part } => match studio.locate(*part) {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::DropPart { part } => match studio.drop_part(*part) {
                true => Outcome::Done,
                false => Outcome::Failed,
            },
            Action::EditAnimation(plan) => match adopt_set(plan, vfs) {
                Some((set, copied, side)) => {
                    studio.adopt(set, plan.target_mod.clone(), plan.motion.clone(), copied, side);

                    Outcome::Opened(Page::Studio)
                }
                None => Outcome::Failed,
            },
            Action::EditFigures(plan) => {
                let subject = plan.subject();
                let bank = &mut self.banks[seat.slot()];
                let already = bank.figures[subject.slot()].drafting();
                let nudge = bank.stacked(seat, subject.page()) - usize::from(already);

                bank.figures[subject.slot()].begin(plan.clone(), nudge, vfs);

                Outcome::Done
            }
            Action::EditGround(plan) => {
                let bank = &mut self.banks[seat.slot()];
                let nudge = usize::from(bank.ground.drafting());

                bank.ground.begin(plan.clone(), nudge, vfs);

                Outcome::Done
            }
            Action::EditProse(plan) => {
                let subject = plan.subject();
                let bank = &mut self.banks[seat.slot()];

                if let Some(aim) = plan.aim() {
                    bank.glossary_aim = Some(aim);
                }

                let already = bank.prose[subject.slot()].drafting();
                let nudge = bank.stacked(seat, subject.page()) - usize::from(already);

                bank.prose[subject.slot()].begin(plan.clone(), nudge, vfs);

                Outcome::Done
            }
            Action::Replace { file, target_mod, game } => {
                replace_image(file, target_mod.as_deref(), game.as_deref())
            }
            Action::Make(making) => match make(making, vfs) {
                true => Outcome::Rescanned,
                false => Outcome::Failed,
            },
            Action::Unmake(making) => match unmake(making, vfs) {
                true => Outcome::Rescanned,
                false => Outcome::Failed,
            },
            Action::Sync { file, target_mod, game } => match mods::place(target_mod, game, file) {
                Ok(path) => {
                    info!(path = %path.display(), "Synced a mod file with game");

                    Outcome::Done
                }
                Err(err) => {
                    warn!(file, "Failed to sync the file with game: {}", err);

                    Outcome::Failed
                }
            },
        }
    }

    fn invoke(&mut self, trail: Trail, vfs: &Vfs, studio: &mut studio::State) -> Task<Message> {
        let Some((actionable, confirms)) = self
            .open
            .as_ref()
            .and_then(|open| pick(&open.items, &trail))
            .map(|item| (item.action.is_some(), item.confirm))
        else {
            return Task::none();
        };

        if !actionable {
            return Task::none();
        }

        if confirms && !self.confirm.armed_for(&trail) {
            return self.confirm.set(trail, Message::ConfirmExpired);
        }

        self.confirm.expire();
        self.failed.expire();

        let Some(open) = self.open.take() else {
            return Task::none();
        };

        let outcome = pick(&open.items, &trail)
            .and_then(|item| item.action.as_ref())
            .map_or(Outcome::Done, |action| self.perform(open.seat, action, vfs, studio));

        match outcome {
            Outcome::Done => Task::none(),
            Outcome::Rescanned => {
                self.rescan = true;

                Task::none()
            }
            Outcome::Opened(page) => {
                self.opened = Some(page);

                Task::none()
            }
            Outcome::Deferred(task) => {
                self.open = Some(open);
                self.pending = Some(trail);

                task
            }
            Outcome::Failed => {
                self.open = Some(open);

                self.failed.set(trail, Message::FailureExpired)
            }
        }
    }
}

fn channel_target(app: &HaganeApp, target: Option<Target>) -> Option<ChannelTarget> {
    let part = match target {
        Some(Target::AnimPart(part)) => Some(part),
        Some(Target::AnimCurve(track)) => app.studio_state.holder(track),
        _ => None,
    };

    Some(ChannelTarget { channels: app.studio_state.channels(part)? })
}

fn expanded(app: &HaganeApp) -> bool {
    if app.current_page == Page::Sandbox {
        return app.sandbox_state.animation_expanded(&app.app_state);
    }

    match scene(app) {
        Page::Cats => app.cat_state.animation_expanded(),
        Page::Enemies => app.enemy_state.animation_expanded(),
        Page::Utilities => app.utilities_state.animation_expanded(),
        Page::Studio => app.studio_state.expanded(),
        _ => false,
    }
}

pub(crate) fn context(app: &HaganeApp, target: Option<Target>) -> Context {
    let broad = app.settings.files.context_scope == ContextScope::Broad;
    let reached = |wanted: Target| target == Some(wanted) || broad;

    if scene(app) == Page::Studio || expanded(app) {
        return Context {
            values: app.settings.files.editor_mode,
            page: scene(app),
            seat: seat(app),
            file: None,
            cats: Vec::new(),
            enemies: Vec::new(),
            icon: None,
            banner: None,
            images: Vec::new(),
            assets: Vec::new(),
            prose: Vec::new(),
            levels: Vec::new(),
            animation: None,
            channels: channel_target(app, target),
            offsets: matches!(target, Some(Target::AnimFields))
                .then(|| app.studio_state.offsets())
                .flatten(),
            ground: None,
            making: None,
            lineup: None,
        };
    }

    let focus = &Focus::held(app);

    Context {
        values: app.settings.files.editor_mode,
        page: scene(app),
        seat: seat(app),
        file: file_target(app, focus, target),
        cats: cat_payloads(app, focus, reached(Target::CatAttributes)),
        enemies: enemy_payloads(app, focus, reached(Target::EnemyAttributes)),
        icon: icon_target(app, focus, icon_subject(focus, target, broad)),
        banner: banner_subject(focus, target, broad).and_then(|id| banner_target(app, focus, id)),
        images: stage_images(app, focus, reached(Target::MapBanner), reached(Target::StageBanner)),
        assets: Vec::new(),
        prose: prose_payloads(app, focus, target, broad),
        levels: level_payloads(&sources(app), focus, reached(Target::CatLevels), reached(Target::CatForms))
            .into_iter()
            .chain(talent_payloads(app, focus, reached(Target::CatTalents)))
            .chain(combo_payloads(app, focus, target, broad))
            .chain(stage_data_payloads(app, focus, reached(Target::StageData) || reached(Target::StageDrops)))
            .chain(map_drop_payloads(app, focus, reached(Target::MapDrops)))
            .chain(drop_chara_payloads(app, focus, reached(Target::StageDrops)))
            .chain(item_buy_payloads(app, focus, target, broad))
            .collect(),
        animation: anim_target(app, focus, reached(Target::CatAnimation), reached(Target::EnemyAnimation)),
        channels: None,
        offsets: None,
        ground: ground_target(app, focus, reached(Target::StageGround)),
        making: make_target(app, focus, reached(Target::StageMake)),
        lineup: match target {
            Some(Target::LineupCell(cell)) if scene(app) == Page::Sandbox => Some(cell),
            _ => None,
        },
    }
}

fn stage_images(app: &HaganeApp, focus: &Focus, map: bool, stage: bool) -> Vec<ImageTarget> {
    if (!map && !stage) || focus.page != Page::Stages {
        return Vec::new();
    }

    let Some(chosen) = focus.stage.as_ref() else {
        return Vec::new();
    };

    let Some(entry) = app.stage_state.data.registry.stages.get(chosen) else {
        return Vec::new();
    };

    let prefix = entry.category.image_prefix();

    let wanted: Vec<(&'static str, String)> = [
        map.then(|| (MAP_BANNER_TITLE, stage_files::map_banner_file(entry.map_id, &prefix))),
        stage.then(|| {
            let file = stage_files::stage_banner_file(&entry.category, entry.map_id, entry.stage_id, &prefix);

            (STAGE_BANNER_TITLE, file)
        }),
    ]
    .into_iter()
    .flatten()
    .collect();

    let bases: Vec<&str> = wanted.iter().map(|(_, key)| key.as_str()).collect();

    variant_sets(&sources(app), focus, &bases)
        .into_iter()
        .zip(wanted)
        .map(|(files, (title, key))| match focus.active_mod.is_some() {
            true => (padded(files, app.vault.vfs.candidates(&key)), title, key),
            false => (files, title, key),
        })
        .filter(|(files, _, _)| !files.is_empty())
        .map(|(files, title, key)| ImageTarget {
            asset: Asset::Variants { key, files },
            title,
            unlocked: focus.unlocked,
            active_mod: focus.active_mod.clone(),
        })
        .collect()
}

fn padded(mut files: Vec<AssetFile>, candidates: Vec<String>) -> Vec<AssetFile> {
    for name in candidates {
        if files.iter().any(|file| file.name == name) {
            continue;
        }

        files.push(AssetFile { name, game: None, mod_copy: None });
    }

    files
}

fn stage_data_payloads(app: &HaganeApp, focus: &Focus, reached: bool) -> Vec<LevelTarget> {
    if !reached {
        return Vec::new();
    }

    stage_data_targets(&sources(app), focus)
}

fn drop_chara_payloads(app: &HaganeApp, focus: &Focus, reached: bool) -> Vec<LevelTarget> {
    if !reached {
        return Vec::new();
    }

    drop_chara_targets(&sources(app), focus)
}

fn map_drop_payloads(app: &HaganeApp, focus: &Focus, reached: bool) -> Vec<LevelTarget> {
    if !reached {
        return Vec::new();
    }

    map_drop_targets(&sources(app), focus)
}

fn item_buy_payloads(app: &HaganeApp, focus: &Focus, target: Option<Target>, broad: bool) -> Vec<LevelTarget> {
    let item = match target {
        Some(Target::TreasureDrop(item)) => Some(item),
        _ if broad || target == Some(Target::StageDrops) => first_drop(&sources(app), focus),
        _ => None,
    };

    item_buy_targets(&sources(app), focus, item)
}

// Creating a stage or a map only makes sense into a mod: the files are new, so there is no
// vanilla original to place and nothing to fall back on under `game`.
fn make_target(app: &HaganeApp, focus: &Focus, reached: bool) -> Option<MakeTarget> {
    if !reached || focus.page != Page::Stages {
        return None;
    }

    let data = &app.stage_state.data;
    let map = focus.map.clone()?;
    let registry = &data.registry;
    let held = registry.addresses.get(&map)?;

    let taken: Vec<u32> = registry
        .stages
        .keys()
        .filter(|key| key.category == map.category && key.map == map.map)
        .map(|key| key.stage)
        .collect();

    let maps: Vec<u32> =
        registry.maps.keys().filter(|key| key.category == map.category).map(|key| key.map).collect();

    // The story chapters number their battlegrounds `stageNormal*.csv` and their names one
    // stage per line in a fixed backwards block, so there is no sibling to renumber and no
    // slot to add. Zombie Outbreaks borrows the same tables and is fixed for the same reason.
    let fixed = names::named_by_stage(&map.category.map_prefix());

    let next_stage = stage_authoring::next_free(&taken, stage_authoring::STAGES_PER_MAP);
    let next_map = stage_authoring::next_free(&maps, stage_authoring::MAPS_PER_CATEGORY);

    let seed = |stage: u32, whole_map: bool, at: u32| -> Option<Making> {
        if fixed {
            return None;
        }

        making(app, focus, &map, held, stage, whole_map, at)
    };

    Some(MakeTarget {
        add_stage: next_stage.and_then(|stage| seed(stage, false, map.map)),
        drop_stage: focus
            .stage
            .as_ref()
            .filter(|chosen| chosen.map == map.map && chosen.category == map.category)
            .and_then(|chosen| seed(chosen.stage, false, map.map)),
        add_map: next_map.and_then(|at| seed(0, true, at)),
        drop_map: seed(0, true, map.map),
        stages_full: next_stage.is_none(),
        maps_full: next_map.is_none(),
        fixed,
    })
}

fn making(
    app: &HaganeApp,
    focus: &Focus,
    map: &GlobalMapId,
    held: &foundry::domains::stage::MapAddress,
    stage: u32,
    whole_map: bool,
    at: u32,
) -> Option<Making> {
    let registry = &app.stage_state.data.registry;
    let prefix = map.category.map_prefix();
    let image = map.category.image_prefix();

    // Names are renumbered off a sibling that already exists, never spelled from a rule:
    // Stories of Legend numbers its maps `N` and its stages `RN`, and nothing but an existing
    // file says so.
    let sibling = registry
        .grounds
        .iter()
        .find(|(key, _)| key.category == map.category)
        .map(|(_, name)| name.to_string())?;

    let ground = stage_authoring::renumbered(&sibling, at, Some(stage))?;
    let data_file = stage_authoring::renumbered(held.data_file.as_deref()?, at, None)?;

    let plate = core::visible_name(&sources(app), &stage_files::stage_banner_file(&map.category, map.map, 0, &image));
    let plate = stage_authoring::renumbered(&plate, at, Some(stage))?;

    let map_plate = core::visible_name(&sources(app), &stage_files::map_banner_file(map.map, &image));
    let map_plate = stage_authoring::renumbered(&map_plate, at, None)?;

    let (name_key, name_cell) = names::stage_name_address(&prefix, at, stage);

    // The scanner finds maps by globbing their data tables and stages by globbing their
    // battlegrounds, and a mod cannot un-glob a file the game ships. Removing a mod's copy of
    // one only uncovers the original, so the thing would not go away.
    let defining = match whole_map { true => &data_file, false => &ground };
    let vanilla = app.vault.vfs.rooted(architecture::GAME, defining).is_some();

    Some(Making {
        target_mod: focus.active_mod.clone(),
        unlocked: focus.unlocked,
        vanilla,
        map: at,
        stage,
        global: map.category.global_map_id(at)?,
        whole_map,
        ground,
        data_file,
        name_file: held.name_file.as_deref()?.to_owned(),
        plate,
        map_plate,
        name_key,
        name_cell,
        story: names::named_by_stage(&prefix),
        label: match whole_map {
            true => format!("Map {at:03}"),
            false => format!("Stage {stage:02}"),
        },
    })
}

fn ground_target(app: &HaganeApp, focus: &Focus, reached: bool) -> Option<GroundTarget> {
    if !reached || focus.page != Page::Stages {
        return None;
    }

    ground_source(app, focus)
}

fn ground_source(app: &HaganeApp, focus: &Focus) -> Option<GroundTarget> {
    let chosen = focus.stage.as_ref()?;
    let file = app.stage_state.data.registry.grounds.get(chosen)?.to_string();
    let game = app.vault.vfs.rooted(architecture::GAME, &file)?;

    let registry = &app.stage_state.data.registry;

    let chapter = registry
        .maps
        .get(&GlobalMapId { category: chosen.category.clone(), map: chosen.map })
        .map(|map| map.name.trim())
        .filter(|name| !name.is_empty());

    let named =
        registry.stages.get(chosen).map(|stage| stage.name.trim()).filter(|name| !name.is_empty());

    let label = chapter
        .into_iter()
        .chain(named)
        .chain(std::iter::once(file.as_str()))
        .collect::<Vec<&str>>()
        .join(theme::HEADER_SEPARATOR);

    let active_mod = focus.active_mod.clone();
    let mod_copy = active_mod.as_deref().and_then(|active| mods::find(&app.vault.vfs, active, &game));

    Some(GroundTarget {
        label,
        file,
        game,
        mod_copy,
        unlocked: focus.unlocked,
        active_mod,
    })
}

fn named_files(app: &HaganeApp, focus: &Focus, names: Vec<String>) -> Vec<AssetFile> {
    let copies: Vec<Option<PathBuf>> = {
        let located = mod_copies(&sources(app), focus, names.iter().map(String::as_str));

        names.iter().map(|name| located.get(name.as_str()).cloned()).collect()
    };

    names
        .into_iter()
        .zip(copies)
        .map(|(name, mod_copy)| {
            let game = app.vault.vfs.rooted(architecture::GAME, &name);

            AssetFile { name, game, mod_copy }
        })
        .collect()
}

fn anim_target(app: &HaganeApp, focus: &Focus, cats: bool, enemies: bool) -> Option<AnimTarget> {
    let vfs = &app.vault.vfs;

    let (files, motion, naming): (_, _, fn(&str) -> Option<&'static str>) = match focus.page {
        Page::Cats if cats => {
            let id = focus.cat?;
            let cat = app.cat_state.data.cats.iter().find(|cat| cat.id == id)?;

            (cat_animation::rig_files(cat, focus.form, vfs)?, held_view(app).selected_motion(), cat_animation::motion_name)
        }
        Page::Enemies if enemies => {
            let id = focus.enemy?;
            let enemy = app.enemy_state.data.enemies.iter().find(|enemy| enemy.id == id)?;

            (enemy_animation::rig_files(enemy, vfs)?, app.enemy_state.selected_motion(), enemy_animation::motion_name)
        }
        _ => return None,
    };

    let key = files.key.clone();
    let asset = Asset::Variants { key: files.key.clone(), files: named_files(app, focus, files.names()) };

    let titles = RIG_TITLES
        .into_iter()
        .map(|title| Some(title.to_owned()))
        .chain(files.anims.iter().map(|(suffix, _)| motion_title(suffix, naming)))
        .collect();

    Some(AnimTarget {
        key,
        motion,
        asset,
        titles,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    })
}

fn motion_title(suffix: &str, naming: fn(&str) -> Option<&'static str>) -> Option<String> {
    if let Some(name) = naming(suffix) {
        return Some(format!("{name} Animation"));
    }

    match suffix {
        ENTRY_SUFFIX => Some("Entry Animation".to_owned()),
        _ if suffix.bytes().all(|byte| byte.is_ascii_digit()) => Some("Extra Animation".to_owned()),
        _ => None,
    }
}

#[derive(Clone)]
pub(crate) struct AnimPlan {
    key: String,
    files: Vec<String>,
    target_mod: Option<String>,
    unlocked: bool,
    motion: Option<String>,
}

fn anim_plan(target: &AnimTarget, target_mod: Option<String>) -> AnimPlan {
    AnimPlan {
        key: target.key.clone(),
        files: target.asset.names(),
        target_mod,
        unlocked: target.unlocked,
        motion: target.motion.clone(),
    }
}
fn adopt_set(plan: &AnimPlan, vfs: &Vfs) -> Option<(studio::Set, bool, Option<Side>)> {
    let mut set = studio::Set { name: plan.key.clone(), ..studio::Set::default() };

    for file in &plan.files {
        let seated = match plan.target_mod.as_deref() {
            Some(name) => stage_into(vfs, name, file),
            None => vfs.rooted(architecture::GAME, file),
        };

        let Some(seated) = seated else {
            continue;
        };

        match seated.extension().and_then(|ext| ext.to_str()) {
            Some("png") => set.sheet = Some(seated),
            Some("imgcut") => set.cuts = Some(seated),
            Some("mamodel") => set.model = Some(seated),
            Some("maanim") => set.anims.push(seated),
            _ => {}
        }
    }

    if !set.rigged() {
        return None;
    }

    let side = plan
        .files
        .iter()
        .find(|file| file.ends_with(".mamodel"))
        .and_then(|file| Path::new(file).file_stem()?.to_str())
        .and_then(foundry_studio::stem_side);

    if plan.target_mod.is_some() || plan.unlocked {
        return Some((set, false, side));
    }

    let name = foundry_studio::vacant(&plan.key);

    match foundry_studio::adopt(&name, &set) {
        Ok(copied) => Some((copied, true, side)),
        Err(err) => {
            warn!(name, "Failed to copy a locked rig into studio: {}", err);

            None
        }
    }
}

fn stage_into(vfs: &Vfs, name: &str, file: &str) -> Option<PathBuf> {
    if let Some(held) = vfs.rooted(name, file) {
        return Some(held);
    }

    let game = vfs.rooted(architecture::GAME, file)?;
    let staged = mods::ensure_as(vfs, name, &game, file)
        .inspect_err(|err| warn!(file, "Failed to copy a rig file into the mod: {}", err))
        .ok()?;

    if let Err(err) = vfs.create((name, staged.as_path())) {
        warn!(path = %staged.display(), "Failed to index a staged rig file: {}", err);
    }

    Some(staged)
}

fn figures_tab(app: &HaganeApp, subject: figures::Subject) -> bool {
    match subject {
        figures::Subject::Cat => held_view(app).selected_tab == DetailTab::Abilities,
        figures::Subject::Enemy => app.enemy_state.selected_tab == EnemyTab::Abilities,
        figures::Subject::Combo => held_view(app).selected_tab == DetailTab::Details,
        figures::Subject::Costs => talents_tab(app),
        figures::Subject::Buy
        | figures::Subject::Curve
        | figures::Subject::Talents
        | figures::Subject::MapStage
        | figures::Subject::MapDrops
        | figures::Subject::DropChara
        | figures::Subject::ItemBuy
        | figures::Subject::MapOption
        | figures::Subject::Background
        | figures::Subject::Upgrade
        | figures::Subject::ComboParam
        | figures::Subject::OrbSlots
        | figures::Subject::Difficulty
        | figures::Subject::ExInvasion
        | figures::Subject::CannonStatus
        | figures::Subject::ExpCurve
        | figures::Subject::OrbGrades
        | figures::Subject::ExLottery
        | figures::Subject::Shakes
        | figures::Subject::Recipes
        | figures::Subject::Shop
        | figures::Subject::ZombieHeader
        | figures::Subject::ZombieTier
        | figures::Subject::Cpu
        | figures::Subject::OrbTraits
        | figures::Subject::Altar
        | figures::Subject::GamatotoLimit
        | figures::Subject::GamatotoUnlock
        | figures::Subject::GamatotoDrop
        | figures::Subject::Stamps
        | figures::Subject::SlotUnlocks
        | figures::Subject::Club
        | figures::Subject::Missions
        | figures::Subject::Conditions
        | figures::Subject::MissionUnlocks
        | figures::Subject::MissionGroups
        | figures::Subject::GachaOptions
        | figures::Subject::ChanceAnimations
        | figures::Subject::Castles
        | figures::Subject::TipRules
        | figures::Subject::QuestStages
        | figures::Subject::SlotLottery
        | figures::Subject::RealmAltar
        | figures::Subject::RealmDraws
        | figures::Subject::LabyrinthFloors
        | figures::Subject::LabyrinthLottery
        | figures::Subject::MissionShortcuts
        | figures::Subject::MissionDesignations
        | figures::Subject::MonthlyMissions
        | figures::Subject::SoundSettings
        | figures::Subject::DojoChests
        | figures::Subject::PopupTriggers
        | figures::Subject::ComboCategories
        | figures::Subject::CastleGrowth
        | figures::Subject::DevelopmentCosts
        | figures::Subject::DevelopmentTiers
        | figures::Subject::MaterialMixes
        | figures::Subject::GuideEntries
        | figures::Subject::UnitLimits
        | figures::Subject::EnemyGuide
        | figures::Subject::ExpeditionAreas
        | figures::Subject::GamatotoLevels
        | figures::Subject::PrizePatterns
        | figures::Subject::ShrineOfferings
        | figures::Subject::ShrineExtras
        | figures::Subject::FortuneOdds
        | figures::Subject::ZonedIn
        | figures::Subject::ZonedInSteps
        | figures::Subject::FilterMaps
        | figures::Subject::LineupExclusions
        | figures::Subject::EnigmaLotteries
        | figures::Subject::EnigmaRates
        | figures::Subject::EnigmaDrops
        | figures::Subject::EnigmaGroups
        | figures::Subject::DiscoveryMedals
        | figures::Subject::MapMarkers
        | figures::Subject::EnigmaTimes
        | figures::Subject::RecommendedUpgrades
        | figures::Subject::LineupCounters
        | figures::Subject::OrbOptions
        | figures::Subject::EnergyRefunds
        | figures::Subject::StageShortcuts
        | figures::Subject::FanfareTiers
        | figures::Subject::GuideSpotlights
        | figures::Subject::TipSettings
        | figures::Subject::EnigmaRarities
        | figures::Subject::EngineerCap
        | figures::Subject::LineupRatings
        | figures::Subject::LineupBands
        | figures::Subject::LineupRankGroups
        | figures::Subject::AbilityIcons
        | figures::Subject::TraitIcons
        | figures::Subject::MenuTextSettings
        | figures::Subject::MapSheets
        | figures::Subject::OrbFilterChoices
        | figures::Subject::ReelSymbols
        | figures::Subject::LevelUpOffers
        | figures::Subject::DemonCastles => true,
    }
}

enum IconSubject {
    Cat(u32),
    Enemy(u32),
}

fn icon_subject(focus: &Focus, target: Option<Target>, broad: bool) -> Option<IconSubject> {
    match target {
        Some(Target::EnemyRow(id)) => Some(IconSubject::Enemy(id)),
        Some(Target::CatIcon) => focus.cat.map(IconSubject::Cat),
        Some(Target::EnemyIcon) => focus.enemy.map(IconSubject::Enemy),
        _ if !broad => None,
        _ => match focus.page {
            Page::Cats => focus.cat.map(IconSubject::Cat),
            Page::Enemies => focus.enemy.map(IconSubject::Enemy),
            _ => None,
        },
    }
}

fn prose_payloads(app: &HaganeApp, focus: &Focus, target: Option<Target>, broad: bool) -> Vec<ProseTarget> {
    if let Some(Target::CatCombo(line)) = target {
        return combo_name_target(&sources(app), focus, Some(line)).into_iter().collect();
    }

    // A list row is the one place a name is offered for something that is not the
    // selection, so the row's own id wins over it -- right-click does not select.
    if let Some(Target::MapRow(map)) = target {
        return map_name_target(&sources(app), focus, Some(map)).into_iter().collect();
    }

    if let Some(Target::StageRow(stage)) = target {
        return stage_name_target(&sources(app), focus, Some(stage)).into_iter().collect();
    }

    if let Some(Target::TreasureDrop(item)) = target {
        return item_name_target(&sources(app), focus, Some(item)).into_iter().collect();
    }

    if let Some(Target::Ability(identity, faction)) = target {
        return glossary_target(&sources(app), focus, identity, faction).into_iter().collect();
    }

    if !broad {
        let Some(subject) = prose_subject(target) else {
            return Vec::new();
        };

        return prose_target(&sources(app), focus, subject).into_iter().collect();
    }

    prose::SUBJECTS
        .into_iter()
        .filter(|subject| subject.page() == focus.page && prose_tab(app, *subject))
        .filter(|subject| prose_offered(app, *subject))
        .flat_map(|subject| prose_target(&sources(app), focus, subject))
        .collect()
}

fn combo_payloads(app: &HaganeApp, focus: &Focus, target: Option<Target>, broad: bool) -> Vec<LevelTarget> {
    if !figures_tab(app, figures::Subject::Combo) {
        return Vec::new();
    }

    let line = match target {
        Some(Target::CatCombo(line)) => Some(line),
        _ if broad || !combo_joined(app, focus) => None,
        _ => return Vec::new(),
    };

    combo_target(&sources(app), focus, line).into_iter().collect()
}

fn combo_joined(app: &HaganeApp, focus: &Focus) -> bool {
    focus.cat.is_some_and(|id| combo_line(&sources(app), id, focus.form).is_some())
}

fn talent_payloads(app: &HaganeApp, focus: &Focus, reached: bool) -> Vec<LevelTarget> {
    if !reached || focus.cat.is_none() {
        return Vec::new();
    }

    let files: &[(figures::Subject, &str)] =
        if talents_tab(app) { &TALENT_FILES } else { &ACQUISITION_ONLY };

    roster_payloads(&sources(app), focus, files)
}

fn talents_tab(app: &HaganeApp) -> bool {
    held_view(app).selected_tab == DetailTab::Talents
}

fn level_cap(app: &HaganeApp, cat: Option<u32>) -> Option<i32> {
    let id = cat?;
    let cat = app.cat_state.data.cats.iter().find(|cat| cat.id == id)?;

    Some(cat.unitbuy.level_cap_catseye + cat.unitbuy.level_cap_plus)
}

fn cat_payloads(app: &HaganeApp, focus: &Focus, reached: bool) -> Vec<CatTarget> {
    if !reached || !figures_tab(app, figures::Subject::Cat) {
        return Vec::new();
    }

    cat_subject(&sources(app), focus).into_iter().collect()
}

fn enemy_payloads(app: &HaganeApp, focus: &Focus, reached: bool) -> Vec<EnemyTarget> {
    if !reached || !figures_tab(app, figures::Subject::Enemy) {
        return Vec::new();
    }

    enemy_subject(&sources(app), focus).into_iter().collect()
}

fn prose_tab(app: &HaganeApp, subject: prose::Subject) -> bool {
    match subject {
        prose::Subject::EnemyDescription => app.enemy_state.selected_tab == EnemyTab::Details,
        prose::Subject::ComboName => held_view(app).selected_tab == DetailTab::Details,
        prose::Subject::TalentText => talents_tab(app),
        prose::Subject::CatGlossary => held_view(app).selected_tab == DetailTab::Abilities,
        prose::Subject::EnemyGlossary => app.enemy_state.selected_tab == EnemyTab::Abilities,
        prose::Subject::Explanation
        | prose::Subject::EnemyName
        | prose::Subject::MapName
        | prose::Subject::StageName
        | prose::Subject::ItemName => true,
    }
}

// Whether a broad right-click offers the subject at all, which is not the same question as
// whether an open popup still renders -- a name popup belongs to the page, so closing the
// sidebar must not hide one. A stage's name is on screen either because its plate is missing
// and the panel drew the name instead, or because the sidebar list is open and its rows are
// names either way. With a plate up and the list closed there is no name anywhere to aim at.
fn prose_offered(app: &HaganeApp, subject: prose::Subject) -> bool {
    match subject {
        prose::Subject::MapName => named(app, Target::MapBanner),
        prose::Subject::StageName => named(app, Target::StageBanner),
        _ => true,
    }
}

fn named(app: &HaganeApp, plate: Target) -> bool {
    app.stage_state.sidebar_open() || !plated(app, plate)
}

fn plated(app: &HaganeApp, plate: Target) -> bool {
    let Some(chosen) = app.stage_state.data.selected_stage.as_ref() else {
        return false;
    };

    let Some(entry) = app.stage_state.data.registry.stages.get(chosen) else {
        return false;
    };

    let prefix = entry.category.image_prefix();

    let wanted = match plate {
        Target::MapBanner => stage_files::map_banner_file(entry.map_id, &prefix),
        _ => stage_files::stage_banner_file(&entry.category, entry.map_id, entry.stage_id, &prefix),
    };

    app.vault.vfs.find(&wanted).is_some()
}

fn prose_subject(target: Option<Target>) -> Option<prose::Subject> {
    match target? {
        Target::CatExplanation => Some(prose::Subject::Explanation),
        Target::EnemyName => Some(prose::Subject::EnemyName),
        Target::EnemyDescription => Some(prose::Subject::EnemyDescription),
        Target::CatCombo(_) => Some(prose::Subject::ComboName),
        Target::CatTalents => Some(prose::Subject::TalentText),
        Target::MapName => Some(prose::Subject::MapName),
        Target::StageName => Some(prose::Subject::StageName),
        Target::TreasureDrop(_) => Some(prose::Subject::ItemName),
        Target::Ability(_, Faction::Cat) => Some(prose::Subject::CatGlossary),
        Target::Ability(_, Faction::Enemy) => Some(prose::Subject::EnemyGlossary),
        _ => None,
    }
}

fn icon_target(app: &HaganeApp, focus: &Focus, subject: Option<IconSubject>) -> Option<IconTarget> {
    let (resolved, title) = match subject? {
        IconSubject::Cat(id) => {
            let cat = app.cat_state.data.cats.iter().find(|cat| cat.id == id)?;

            (cat.deploy_icon_paths.get(focus.form)?.as_ref()?, CAT_ICON_TITLE)
        }
        IconSubject::Enemy(id) => {
            let enemy = app.enemy_state.data.enemies.iter().find(|enemy| enemy.id == id)?;

            (enemy.icon_path.as_ref()?, ENEMY_ICON_TITLE)
        }
    };

    let name = resolved.file_name()?.to_string_lossy().into_owned();

    Some(IconTarget {
        asset: exception(&sources(app), focus, name)?,
        title,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    })
}

fn banner_subject(focus: &Focus, target: Option<Target>, broad: bool) -> Option<u32> {
    match target {
        Some(Target::CatRow(id)) => Some(id),
        _ if !broad => None,
        _ => focus.cat,
    }
}

fn banner_target(app: &HaganeApp, focus: &Focus, id: u32) -> Option<BannerTarget> {
    if focus.page != Page::Cats {
        return None;
    }

    let cat = app.cat_state.data.cats.iter().find(|cat| cat.id == id)?;
    let egg_ids = cat.egg_ids.unwrap_or(NO_EGGS);

    let wanted: Vec<String> =
        (0..cat.forms.len()).map(|form| cat_files::banner_file(id, form, egg_ids)).collect();

    let forms: Vec<(Option<&'static str>, Exception)> = exceptions(&sources(app), focus, wanted)
        .into_iter()
        .enumerate()
        .filter_map(|(form, asset)| {
            let offered = form == NORMAL_FORM || cat.forms[form] || !asset.variants.is_empty();
            offered.then(|| (BANNER_TITLES.get(form).copied(), asset))
        })
        .collect();

    Some(BannerTarget {
        key: cat_files::banner_base(id),
        forms,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    })
}

// Creating a stage writes every file it needs, placeholders included: the engine faults on a
// missing asset, so a stage that exists in one table and nowhere else is worse than no stage.
// Every file the thing needs, placeholders included. Pure so the list can be checked without
// a filesystem: a stage that reaches one table and no assets is the crash this exists to stop.
fn authored(making: &Making) -> Vec<(String, Vec<u8>)> {
    let mut written = Vec::new();

    if making.whole_map {
        for held in stage_authoring::blank_plate(&making.map_plate) {
            written.push((held.name, held.bytes));
        }

        written.push((making.data_file.clone(), stage_authoring::blank_map_data(making.global)));
    }

    // A map is born holding its first stage, so both paths write a stage's own files.
    for held in stage_authoring::blank_plate(&making.plate) {
        written.push((held.name, held.bytes));
    }

    written.push((making.ground.clone(), stage_authoring::blank_battleground()));

    written
}

fn make(making: &Making, vfs: &Vfs) -> bool {
    let Some(mod_name) = making.target_mod.as_deref() else {
        warn!("Creating a stage needs an active mod; the game mount has no place to put one");

        return false;
    };

    for (name, bytes) in authored(making) {
        let Ok(path) = mods::create(mod_name, &name, &bytes) else {
            return false;
        };

        // Published straight away: the rescan reads through the index, and the watcher's
        // debounce would not have landed by then.
        if let Err(err) = vfs.create((mod_name, path.as_path())) {
            warn!(name, "Wrote a file the index would not take: {}", err);
        }
    }

    if making.whole_map {
        return option_row(making, vfs, true) && name_seated(making, vfs, true);
    }

    data_row(making, vfs, true) && name_seated(making, vfs, true)
}

// Removal is confined to what the mod itself holds. A vanilla stage's files live under the
// game mount and unpicking one there is a different, destructive operation.
fn unmake(making: &Making, vfs: &Vfs) -> bool {
    let mount = making.mount();

    // A map takes its own table with it; a stage leaves the table it shared and only its own
    // files go.
    let held = match making.whole_map {
        true => vec![making.map_plate.clone(), making.data_file.clone()],
        false => vec![
            making.plate.clone(),
            stage_authoring::cut_name(&making.plate),
            making.ground.clone(),
        ],
    };

    for name in held {
        let Some(path) = vfs.rooted(mount, &name) else { continue };

        if let Err(err) = fs::remove_file(&path) {
            warn!(path = %path.display(), "Could not remove a file: {}", err);

            continue;
        }

        vfs.destroy((mount, path.as_path()));
    }

    if making.whole_map {
        return option_row(making, vfs, false) && name_seated(making, vfs, false);
    }

    data_row(making, vfs, false) && name_seated(making, vfs, false)
}

// The shared tables are copied into the mod on first touch, exactly as the row editors do,
// so a table the mod has not adopted yet is never edited in place under `game`.
fn staged(vfs: &Vfs, target_mod: Option<&str>, name: &str) -> Option<(PathBuf, String)> {
    let path = match target_mod {
        Some(mod_name) => {
            let source = vfs.find(name)?;

            mods::ensure_as(vfs, mod_name, &source, name)
                .inspect_err(|err| warn!(name, "Could not stage a table into the mod: {}", err))
                .ok()?
        }
        None => vfs.rooted(architecture::GAME, name)?,
    };

    let bytes = fs::read(&path).ok()?;

    Some((path, common::scrub(&bytes)))
}

fn rewrite(path: &Path, body: &str) -> bool {
    fs::write(path, body.as_bytes())
        .inspect_err(|err| warn!(path = %path.display(), "Could not write a table: {}", err))
        .is_ok()
}

fn data_row(making: &Making, vfs: &Vfs, adding: bool) -> bool {
    let Some((path, body)) = staged(vfs, making.target_mod.as_deref(), &making.data_file) else {
        return false;
    };

    let at = figures::MAP_HEADER_LINES + making.stage as usize;

    // Emptied rather than closed up: a stage's row is its line, so removing one would shift
    // every later stage's data a seat up.
    let body = match adding {
        true => stage_authoring::with_line(&body, at, &stage_authoring::blank_stage_row()),
        false => stage_authoring::blanked_line(&body, at),
    };

    rewrite(&path, &body)
}

// A story chapter names one stage per line; every other chapter holds a whole map on one
// line and names a stage by its cell. `stage_name_rows` is the same scan the name editor
// addresses through, so the two cannot land on different lines.
fn name_seat(making: &Making, body: &str, delimiter: char) -> usize {
    names::stage_name_rows(body, delimiter)
        .get(&making.name_key)
        .copied()
        .unwrap_or(making.name_key as usize)
}

fn name_seated(making: &Making, vfs: &Vfs, adding: bool) -> bool {
    let Some((path, body)) = staged(vfs, making.target_mod.as_deref(), &making.name_file) else {
        return false;
    };

    let delimiter = prose::separator(&making.name_file);
    let at = name_seat(making, &body, delimiter);

    // A map is a line here and a stage a cell of it, both addressed by position, so removal
    // empties rather than closes up for the same reason the data table does.
    let body = match (adding, making.story) {
        (true, true) => stage_authoring::with_line(&body, at, &making.label),
        (false, true) => stage_authoring::blanked_line(&body, at),
        (true, false) => {
            stage_authoring::with_cell(&body, delimiter, at, making.name_cell, &making.label)
        }
        (false, false) => stage_authoring::blanked_cell(&body, delimiter, at, making.name_cell),
    };

    rewrite(&path, &body)
}

fn option_row(making: &Making, vfs: &Vfs, adding: bool) -> bool {
    let Some((path, body)) = staged(vfs, making.target_mod.as_deref(), stage_files::MAP_OPTION) else {
        return false;
    };

    let found = stage_authoring::line_of(&body, ',', 0, making.global);

    let body = match (adding, found) {
        (true, Some(_)) => return true,
        (true, None) => {
            let at = body.lines().count();

            stage_authoring::with_line(&body, at, &stage_authoring::blank_map_option(making.global))
        }
        (false, Some(at)) => stage_authoring::without_line(&body, at),
        (false, None) => return true,
    };

    rewrite(&path, &body)
}

fn replace_image(file: &str, target_mod: Option<&str>, game: Option<&Path>) -> Outcome {
    let file = file.to_string();
    let target_mod = target_mod.map(str::to_string);
    let game = game.map(Path::to_path_buf);

    Outcome::Deferred(Task::perform(
        async move {
            let Some(source) = dialog::file("PNG Image", &["png"]).await else { return true; };

            place_image(&file, target_mod.as_deref(), game.as_deref(), &source)
        },
        Message::Replaced,
    ))
}

fn place_image(file: &str, target_mod: Option<&str>, game: Option<&Path>, source: &Path) -> bool {
    let placed = match target_mod {
        Some(name) => mods::place(name, source, file),
        None => match game {
            Some(path) => fs::copy(source, path).map(|_| path.to_path_buf()),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "the vanilla image is missing")),
        },
    };

    placed
        .inspect(|path| info!(path = %path.display(), "Replaced an image"))
        .inspect_err(|err| warn!(file, "Failed to replace the image: {}", err))
        .is_ok()
}

pub(crate) fn refresh(app: &HaganeApp, editor: &State, force: bool) -> Option<Update> {
    if !editor.drafting() {
        return None;
    }

    let key = key(app);

    if !force && !editor.stale(&key) {
        return None;
    }

    Some(Update { snapshot: snapshot(app, editor), key })
}

fn key(app: &HaganeApp) -> Key {
    Key { seat: seat(app), values: app.settings.files.editor_mode, focus: Focus::held(app) }
}

fn snapshot(app: &HaganeApp, editor: &State) -> Snapshot {
    let seat = seat(app);

    if seat == Seat::Mods {
        return pinned_snapshot(app, &editor.banks[seat.slot()]);
    }

    let focus = &Focus::held(app);

    Snapshot {
        page: focus.page,
        seat,
        figures: figures::SUBJECTS.map(|subject| {
            if subject.page() != focus.page || !editor.figures_drafting(seat, subject) {
                return None;
            }

            current_plan(&sources(app), focus, subject, Aim::default(), app.settings.files.editor_mode)
        }),
        prose: prose::SUBJECTS.map(|subject| {
            if subject.page() != focus.page || !editor.prose_drafting(seat, subject) {
                return Vec::new();
            }

            let target = match (subject, editor.glossary_aim(seat)) {
                (prose::Subject::CatGlossary | prose::Subject::EnemyGlossary, Some((identity, faction))) => glossary_target(&sources(app), focus, identity, faction),
                _ => prose_target(&sources(app), focus, subject),
            };

            target.map(|target| prose_plans(&target)).unwrap_or_default()
        }),
        ground: current_ground(app, focus),
        artwork: None,
        sheet: None,
        ledger: None,
        records: None,
        preset: None,
        group: None,
        treasure: None,
        gift: None,
        filter: None,
        evolve: None,
        roster: None,
        captions: None,
        catfruit: None,
        maplayout: None,
        dungeon: None,
        cuts: None,
        logins: None,
        unused: None,
        audio: None,
    }
}

fn pinned_snapshot(app: &HaganeApp, bank: &Bank) -> Snapshot {
    let values = app.settings.files.editor_mode;
    let planned = |pin: &Option<Pick>| pin.as_ref().and_then(|pick| pick_plan(app, pick, values));

    Snapshot {
        page: Page::Mods,
        seat: Seat::Mods,
        figures: figures::SUBJECTS.map(|subject| match planned(&bank.pins.figures[subject.slot()]) {
            Some(Planned::Figures(plan)) => Some(plan),
            _ => None,
        }),
        prose: prose::SUBJECTS.map(|subject| match planned(&bank.pins.prose[subject.slot()]) {
            Some(Planned::Prose(plan)) => vec![plan],
            _ => Vec::new(),
        }),
        ground: match planned(&bank.pins.ground) {
            Some(Planned::Ground(plan)) => Some(plan),
            _ => None,
        },
        artwork: match planned(&bank.pins.artwork) {
            Some(Planned::Artwork(plan)) => Some(plan),
            _ => None,
        },
        sheet: match planned(&bank.pins.sheet) {
            Some(Planned::Sheet(plan)) => Some(plan),
            _ => None,
        },
        ledger: match planned(&bank.pins.ledger) {
            Some(Planned::Ledger(plan)) => Some(plan),
            _ => None,
        },
        records: match planned(&bank.pins.records) {
            Some(Planned::Records(plan)) => Some(plan),
            _ => None,
        },
        preset: match planned(&bank.pins.preset) {
            Some(Planned::Preset(plan)) => Some(plan),
            _ => None,
        },
        group: match planned(&bank.pins.group) {
            Some(Planned::Group(plan)) => Some(plan),
            _ => None,
        },
        treasure: match planned(&bank.pins.treasure) {
            Some(Planned::Treasure(plan)) => Some(plan),
            _ => None,
        },
        gift: match planned(&bank.pins.gift) {
            Some(Planned::Gift(plan)) => Some(plan),
            _ => None,
        },
        filter: match planned(&bank.pins.filter) {
            Some(Planned::Filter(plan)) => Some(plan),
            _ => None,
        },
        evolve: match planned(&bank.pins.evolve) {
            Some(Planned::Evolve(plan)) => Some(plan),
            _ => None,
        },
        roster: match planned(&bank.pins.roster) {
            Some(Planned::Roster(plan)) => Some(plan),
            _ => None,
        },
        captions: match planned(&bank.pins.captions) {
            Some(Planned::Captions(plan)) => Some(plan),
            _ => None,
        },
        catfruit: match planned(&bank.pins.catfruit) {
            Some(Planned::Catfruit(plan)) => Some(plan),
            _ => None,
        },
        maplayout: match planned(&bank.pins.maplayout) {
            Some(Planned::MapLayout(plan)) => Some(plan),
            _ => None,
        },
        dungeon: match planned(&bank.pins.dungeon) {
            Some(Planned::Dungeon(plan)) => Some(plan),
            _ => None,
        },
        cuts: match planned(&bank.pins.cuts) {
            Some(Planned::Cuts(plan)) => Some(plan),
            _ => None,
        },
        logins: match planned(&bank.pins.logins) {
            Some(Planned::Logins(plan)) => Some(plan),
            _ => None,
        },
        unused: match planned(&bank.pins.unused) {
            Some(Planned::Unused(plan)) => Some(plan),
            _ => None,
        },
        audio: match planned(&bank.pins.audio) {
            Some(Planned::Audio(plan)) => Some(plan),
            _ => None,
        },
    }
}

fn current_ground(app: &HaganeApp, focus: &Focus) -> Option<ground::Plan> {
    let target = ground_source(app, focus)?;

    Some(ground::plan(
        target.label,
        &target.game,
        target.active_mod,
        app.settings.files.editor_mode,
    ))
}

impl Paged for Art {
    fn page(self) -> Page {
        paged(self.realm())
    }
}

impl Paged for Pane {
    fn page(self) -> Page {
        paged(self.realm())
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Pick {
    pane: Pane,
    focus: Focus,
    key: Option<u32>,
    line: Option<usize>,
    aim: Option<(Identity, Faction)>,
    file: String,
    label: String,
    path: Vec<String>,
}

impl Pick {
    pub(crate) fn loose(&self) -> bool {
        self.pane == Pane::File
    }

    pub(crate) fn file(&self) -> &str {
        &self.file
    }
}

pub(crate) enum Planned {
    Figures(figures::Plan),
    Prose(prose::Plan),
    Ground(ground::Plan),
    Artwork(artwork::Plan),
    Sheet(sheet::Plan),
    Ledger(ledger::Plan),
    Records(records::Plan),
    Preset(preset::Plan),
    Group(group::Plan),
    Treasure(treasure::Plan),
    Gift(gift::Plan),
    Filter(filter::Plan),
    Evolve(evolve::Plan),
    Roster(roster::Plan),
    Captions(captions::Plan),
    Catfruit(catfruit::Plan),
    MapLayout(maplayout::Plan),
    Dungeon(dungeon::Plan),
    Cuts(cuts::Plan),
    Logins(logins::Plan),
    Unused(unused::Plan),
    Audio(audio::Plan),
}

impl Planned {
    fn mark(&self, vfs: &Vfs, reader: &mut Reader) -> Mark {
        match self {
            Planned::Figures(plan) => figures::mark(plan, vfs, reader),
            Planned::Prose(plan) => prose::mark(plan, vfs, reader),
            Planned::Ground(plan) => ground::mark(plan, vfs, reader),
            Planned::Artwork(plan) => artwork::mark(plan, vfs),
            Planned::Sheet(plan) => sheet::mark(plan, vfs, reader),
            Planned::Ledger(plan) => ledger::mark(plan, vfs, reader),
            Planned::Records(plan) => records::mark(plan, vfs, reader),
            Planned::Preset(plan) => preset::mark(plan, vfs, reader),
            Planned::Group(plan) => group::mark(plan, vfs, reader),
            Planned::Treasure(plan) => treasure::mark(plan, vfs, reader),
            Planned::Gift(plan) => gift::mark(plan, vfs, reader),
            Planned::Filter(plan) => filter::mark(plan, vfs, reader),
            Planned::Evolve(plan) => evolve::mark(plan, vfs, reader),
            Planned::Roster(plan) => roster::mark(plan, vfs, reader),
            Planned::Captions(plan) => captions::mark(plan, vfs, reader),
            Planned::Catfruit(plan) => catfruit::mark(plan, vfs, reader),
            Planned::MapLayout(plan) => maplayout::mark(plan, vfs, reader),
            Planned::Dungeon(plan) => dungeon::mark(plan, vfs, reader),
            Planned::Cuts(plan) => cuts::mark(plan, vfs, reader),
            Planned::Logins(plan) => logins::mark(plan, vfs, reader),
            Planned::Unused(_) => unused::mark(),
            Planned::Audio(plan) => audio::mark(plan, vfs),
        }
    }

    fn reset(self, vfs: &Vfs) -> bool {
        match self {
            Planned::Figures(plan) => figures::reset(plan, vfs),
            Planned::Prose(plan) => prose::reset(plan, vfs),
            Planned::Ground(plan) => ground::reset(plan, vfs),
            Planned::Artwork(plan) => artwork::reset(&plan, vfs),
            Planned::Sheet(plan) => sheet::reset(plan, vfs),
            Planned::Ledger(plan) => ledger::reset(plan, vfs),
            Planned::Records(plan) => records::reset(plan, vfs),
            Planned::Preset(plan) => preset::reset(plan, vfs),
            Planned::Group(plan) => group::reset(plan, vfs),
            Planned::Treasure(plan) => treasure::reset(plan, vfs),
            Planned::Gift(plan) => gift::reset(plan, vfs),
            Planned::Filter(plan) => filter::reset(plan, vfs),
            Planned::Evolve(plan) => evolve::reset(plan, vfs),
            Planned::Roster(plan) => roster::reset(plan, vfs),
            Planned::Captions(plan) => captions::reset(plan, vfs),
            Planned::Catfruit(plan) => catfruit::reset(plan, vfs),
            Planned::MapLayout(plan) => maplayout::reset(plan, vfs),
            Planned::Dungeon(plan) => dungeon::reset(plan, vfs),
            Planned::Cuts(plan) => cuts::reset(plan, vfs),
            Planned::Logins(plan) => logins::reset(plan, vfs),
            Planned::Unused(_) => false,
            Planned::Audio(plan) => audio::reset(&plan, vfs),
        }
    }
}

pub(crate) fn marks(app: &HaganeApp, leaves: &[u32], target: &str) -> Vec<(u32, Mark)> {
    let values = app.settings.files.editor_mode;
    let vfs = &app.vault.vfs;
    let mut reader = Reader::default();

    leaves
        .iter()
        .map(|leaf| {
            let mark = app
                .editor
                .finder
                .pick(*leaf, target.to_owned())
                .and_then(|pick| match pick.loose() {
                    true => Some(loose_mark(vfs, &pick)),
                    false => pick_plan(app, &pick, values).map(|planned| planned.mark(vfs, &mut reader)),
                })
                .unwrap_or(Mark::Same);

            (*leaf, mark)
        })
        .collect()
}

fn loose_mark(vfs: &Vfs, pick: &Pick) -> Mark {
    pick.focus.active_mod.as_deref().map_or(Mark::Same, |target| core::file_mark(vfs, target, &pick.file))
}

pub(crate) fn place_loose(app: &HaganeApp, pick: &Pick) -> Option<String> {
    let vfs = &app.vault.vfs;
    let target = pick.focus.active_mod.clone()?;
    let source = vfs.rooted(architecture::GAME, &pick.file).or_else(|| vfs.find(pick.file.as_str()))?;

    let placed = mods::ensure_as(vfs, &target, &source, &pick.file)
        .inspect_err(|err| warn!(file = pick.file, "The file could not be copied into the mod: {}", err))
        .ok()?;

    if let Err(err) = vfs.create((target.as_str(), placed.as_path())) {
        warn!(path = %placed.display(), "Failed to index the mod's new file: {}", err);
    }

    Some(target)
}

pub(crate) fn drop_loose(app: &HaganeApp, pick: &Pick) -> bool {
    pick.focus.active_mod.as_deref().is_some_and(|target| core::file_reset(&app.vault.vfs, target, &pick.file))
}

fn pick_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let focus = &pick.focus;

    match pick.pane {
        Pane::File => None,
        Pane::Figures(subject) => {
            let aim = Aim { key: pick.key, line: pick.line, file: Some(&pick.file) };

            current_plan(&sources(app), focus, subject, aim, app.settings.files.editor_mode).map(Planned::Figures)
        }
        Pane::Prose(subject) => {
            let target = match subject {
                prose::Subject::CatGlossary | prose::Subject::EnemyGlossary => {
                    let (identity, faction) = pick.aim?;

                    glossary_target(&sources(app), focus, identity, faction)
                }
                prose::Subject::ItemName => item_name_target(&sources(app), focus, pick.key),
                prose::Subject::ComboName => combo_name_target(&sources(app), focus, pick.line),
                _ => prose_target(&sources(app), focus, subject),
            }?;

            let mut plans = prose_plans(&target);
            let at = plans.iter().position(|plan| plan.file() == pick.file)?;

            Some(Planned::Prose(plans.swap_remove(at)))
        }
        Pane::Ground => {
            let target = ground_source(app, focus)?;

            Some(Planned::Ground(ground::plan(target.label, &target.game, target.active_mod, values)))
        }
        Pane::Artwork(_) => {
            let target = focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str());

            Some(Planned::Artwork(artwork::plan(pick.label.clone(), pick.file.clone(), game, target)))
        }
        Pane::Sheet(kind) => {
            let target = focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str())?;

            Some(Planned::Sheet(sheet::plan(kind, pick.label.clone(), pick.file.clone(), game, target, values)))
        }
        Pane::Ledger(kind) => ledger_plan(app, pick, kind, values),
        Pane::Preset => preset_plan(app, pick, values),
        Pane::Group => group_plan(app, pick, values),
        Pane::Treasure => treasure_plan(app, pick, values),
        Pane::Gift => gift_plan(app, pick, values),
        Pane::Filter => filter_plan(app, pick, values),
        Pane::Evolve => evolve_plan(app, pick, values),
        Pane::Roster(tier) => roster_plan(app, pick, tier, values),
        Pane::Captions(layout) => captions_plan(app, pick, layout, values),
        Pane::Catfruit => catfruit_plan(app, pick, values),
        Pane::Unused => Some(Planned::Unused(unused::plan(pick.label.clone(), pick.file.as_str()))),
        Pane::Audio => {
            let target = pick.focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str());

            Some(Planned::Audio(audio::plan(pick.label.clone(), pick.file.clone(), game, target)))
        }
        Pane::Logins => {
            let target = pick.focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str())?;

            Some(Planned::Logins(logins::plan(pick.label.clone(), pick.file.clone(), game, target, values, pick.line?)))
        }
        Pane::Cuts => {
            let target = pick.focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str())?;

            Some(Planned::Cuts(cuts::plan(pick.label.clone(), pick.file.clone(), game, target)))
        }
        Pane::MapLayout => {
            let target = pick.focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str())?;

            Some(Planned::MapLayout(maplayout::plan(pick.label.clone(), pick.file.clone(), game, target, values)))
        }
        Pane::Dungeon => {
            let target = pick.focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str())?;

            Some(Planned::Dungeon(dungeon::plan(pick.label.clone(), pick.file.clone(), game, target, values)))
        }
        Pane::Records(book) => {
            let target = focus.active_mod.clone()?;
            let game = app.vault.vfs.pristine(pick.file.as_str())?;

            let path = match (pick.path.is_empty().then(|| book.map_root()).flatten(), focus.map.as_ref()) {
                (Some(root), Some(map)) => {
                    let global = app.stage_state.data.registry.addresses.get(map)?.global?;

                    vec![root.to_owned(), global.to_string()]
                }
                _ => pick.path.clone(),
            };

            Some(Planned::Records(records::plan(records::Seed {
                book,
                label: pick.label.clone(),
                file: pick.file.clone(),
                game,
                target_mod: target,
                path,
                values,
                named: match book {
                    records::Book::Orbs => orb_named(app),
                    records::Book::ClearRewards => reward_named(app),
                    _ => Vec::new(),
                },
            })))
        }
    }
}

fn ledger_plan(app: &HaganeApp, pick: &Pick, kind: ledger::Ledger, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    let (key, stages) = match kind {
        ledger::Ledger::Growth(_) => (pick.key?, Vec::new()),
        ledger::Ledger::StageOption
        | ledger::Ledger::FixedFormation
        | ledger::Ledger::StageDraws
        | ledger::Ledger::Checkpoints
        | ledger::Ledger::StageLimits
        | ledger::Ledger::ExDraws
        | ledger::Ledger::CpuLocks => map_stages(app, pick)?,
    };

    let presets = match kind {
        ledger::Ledger::FixedFormation => app
            .vault
            .vfs
            .glob(PRESET_PREFIX)
            .into_iter()
            .filter(|name| name.ends_with(PRESET_EXTENSION))
            .map(String::from)
            .collect(),
        ledger::Ledger::StageOption
        | ledger::Ledger::StageDraws
        | ledger::Ledger::Checkpoints
        | ledger::Ledger::StageLimits
        | ledger::Ledger::ExDraws
        | ledger::Ledger::CpuLocks
        | ledger::Ledger::Growth(_) => Vec::new(),
    };

    let effects = match kind {
        ledger::Ledger::Growth(assembly) => growth_effects(app, assembly),
        ledger::Ledger::ExDraws => ex_groups(app),
        ledger::Ledger::StageOption
        | ledger::Ledger::FixedFormation
        | ledger::Ledger::StageDraws
        | ledger::Ledger::Checkpoints
        | ledger::Ledger::StageLimits
        | ledger::Ledger::CpuLocks => Vec::new(),
    };

    Some(Planned::Ledger(ledger::plan(ledger::Seed {
        ledger: kind,
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        key,
        stages,
        presets,
        effects,
        values,
    })))
}

fn map_stages(app: &HaganeApp, pick: &Pick) -> Option<(u32, Vec<(u32, String)>)> {
    let map = pick.focus.map.as_ref()?;
    let registry = &app.stage_state.data.registry;
    let key = registry.addresses.get(map)?.global?;

    let mut stages: Vec<(u32, String)> = registry
        .stages
        .iter()
        .filter(|(id, _)| id.category == map.category && id.map == map.map)
        .map(|(id, stage)| {
            let named = stage.name.trim();
            let label = if named.is_empty() { format!("Stage {:02}", id.stage) } else { format!("{:02} {named}", id.stage) };

            (id.stage, label)
        })
        .collect();

    stages.sort_unstable_by_key(|(stage, _)| *stage);

    Some((key, stages))
}

fn ex_groups(app: &HaganeApp) -> Vec<(i32, String)> {
    let Some(bytes) = app.vault.vfs.load(EX_LOTTERY_TABLE) else {
        return Vec::new();
    };

    let mut groups = Vec::new();

    for line in common::scrub(&bytes).lines() {
        let mut cells = line.split(',').map(str::trim);

        let (Some(map), Some(stage)) = (cells.next().and_then(|cell| cell.parse::<i32>().ok()), cells.next().and_then(|cell| cell.parse::<i32>().ok())) else {
            break;
        };

        let group = i32::try_from(groups.len()).unwrap_or(i32::MAX);

        groups.push((group, format!("{EX_GROUP} {group}: {EX_MAP} {map}, {EX_STAGE} {stage:02}")));
    }

    groups
}

fn growth_effects(app: &HaganeApp, assembly: ledger::Assembly) -> Vec<(i32, String)> {
    let names = &app.names;
    let numbered = |at: usize| i32::try_from(at).unwrap_or(i32::MAX);

    let named = |kind: usize| match assembly {
        ledger::Assembly::Cannon => base::CANNON_EFFECTS.get(kind).map(|name| (*name).to_owned()),
        ledger::Assembly::Foundation => base::foundation_effect(kind, names).map(str::to_owned),
        ledger::Assembly::Style => base::style_effect(kind, names).map(str::to_owned),
    };

    (0..).map_while(|kind| named(kind).map(|name| (numbered(kind), name))).collect()
}

fn preset_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    let roster = app
        .cat_state
        .data
        .cats
        .iter()
        .map(|cat| preset::Member {
            id: cat.id,
            forms: (0..cat.forms.len()).filter(|form| cat.forms[*form]).map(|form| cat.display_name(form)).collect(),
            caps: cat.unitbuy.level_caps().map(|(base, plus)| (i64::from(base), i64::from(plus))),
        })
        .collect();

    let cannons = (0..)
        .zip(PRESET_CANNONS)
        .map(|(part, fallback)| app.names.cannon(part).unwrap_or(fallback).to_owned())
        .collect();

    Some(Planned::Preset(preset::plan(preset::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        roster,
        cannons,
    })))
}

fn reward_named(app: &HaganeApp) -> records::Named {
    let catalog = app.vault.vds.items.catalog(&app.vault.vfs);
    let mut ids: Vec<u32> = catalog.iter().filter_map(|row| u32::try_from(row.stage_drop_item_id).ok()).collect();

    ids.sort_unstable();
    ids.dedup();

    let mut items = vec![(NO_ITEM, NO_ITEM_LABEL.to_owned())];

    items.extend(ids.into_iter().filter_map(|id| Some((i32::try_from(id).ok()?, core::item_label(&sources(app), id)))));

    vec![(REWARD_ITEM, items)]
}

fn orb_named(app: &HaganeApp) -> records::Named {
    core::orb_named(&sources(app))
}

fn evolve_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let line = usize::try_from(pick.focus.cat?).ok()?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    Some(Planned::Evolve(evolve::plan(evolve::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        line,
    })))
}

fn catfruit_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let key = pick.key?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    let mut ids: Vec<u32> = app.vault.vds.items.catalog(&app.vault.vfs).iter().filter_map(|row| u32::try_from(row.stage_drop_item_id).ok()).collect();

    ids.sort_unstable();
    ids.dedup();

    let items = ids.into_iter().filter_map(|id| Some((id, app.vault.vds.items.name(&app.vault.vfs, id)?))).collect();

    Some(Planned::Catfruit(catfruit::plan(catfruit::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        key,
        items,
    })))
}

fn captions_plan(app: &HaganeApp, pick: &Pick, layout: captions::Layout, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    let key = match (layout.keyed(), pick.line) {
        (true, _) => pick.key?,
        (false, Some(line)) => u32::try_from(line).ok()?,
        (false, None) => pick.focus.cat?,
    };

    Some(Planned::Captions(captions::plan(captions::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        layout,
        key,
    })))
}

fn roster_plan(app: &HaganeApp, pick: &Pick, tier: roster::Tier, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let line = pick.line?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;
    let roster = app.cat_state.data.cats.iter().map(|cat| (cat.id, cat.display_name(0))).collect();

    Some(Planned::Roster(roster::plan(roster::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        tier,
        line,
        roster,
    })))
}

fn filter_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let line = pick.line?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    let effects = app
        .vault
        .vds
        .cats
        .combo_effects(&app.vault.vfs)
        .iter()
        .enumerate()
        .filter_map(|(at, name)| {
            let name = name.as_deref()?.trim();

            (!name.is_empty()).then(|| (i64::try_from(at).unwrap_or(i64::MAX), name.to_owned()))
        })
        .collect();

    Some(Planned::Filter(filter::plan(filter::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        line,
        effects,
    })))
}

fn gift_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let rank = pick.key?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;
    let catalog = app.vault.vds.items.catalog(&app.vault.vfs);

    let mut ids: Vec<u32> = catalog.iter().filter_map(|row| u32::try_from(row.stage_drop_item_id).ok()).collect();

    ids.sort_unstable();
    ids.dedup();

    let items = ids.into_iter().map(|id| (i64::from(id), core::item_label(&sources(app), id))).collect();

    Some(Planned::Gift(gift::plan(gift::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        rank,
        items,
    })))
}

fn treasure_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let group = pick.line?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;
    let slot = treasure::ARCS.iter().map(|(slot, _)| *slot).find(|slot| data_file(*slot).as_deref() == Some(pick.file.as_str()))?;
    let catalog = app.vault.vds.treasures.catalog(&app.vault.vfs);

    let items = (0..TREASURE_STAGES)
        .map(|stage| i32::try_from(stage).ok().and_then(|stage| catalog.item(slot, stage)).unwrap_or_default().to_owned())
        .collect();

    Some(Planned::Treasure(treasure::plan(treasure::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        group,
        items,
    })))
}

fn group_plan(app: &HaganeApp, pick: &Pick, values: EditorMode) -> Option<Planned> {
    let target = pick.focus.active_mod.clone()?;
    let key = pick.key?;
    let game = app.vault.vfs.pristine(pick.file.as_str())?;

    let roster = app.cat_state.data.cats.iter().map(|cat| (cat.id, cat.display_name(0))).collect();

    let described = app.vault.vds.stages.charagroups(&app.vault.vfs).get(&key).and_then(|group| {
        let text_key = group.text_id.clone()?;
        let words = app.localizable.lookup(&text_key)?.trim().to_owned();

        Some((text_key, words))
    });

    Some(Planned::Group(group::plan(group::Seed {
        label: pick.label.clone(),
        file: pick.file.clone(),
        game,
        target_mod: target,
        values,
        key,
        roster,
        described,
    })))
}

pub(crate) fn chosen(app: &HaganeApp, pick: &Pick) -> Option<Planned> {
    pick_plan(app, pick, app.settings.files.editor_mode)
}

pub(crate) fn sources(app: &HaganeApp) -> core::Sources<'_> {
    core::Sources {
        vault: &app.vault,
        cats: &app.cat_state.data.cats,
        enemies: &app.enemy_state.data.enemies,
        registry: &app.stage_state.data.registry,
        drops: &app.stage_state.data.drop_chara_registry,
        cat_names: &app.stage_state.data.cat_name_registry,
        names: &app.names,
        localizable: &app.localizable,
        category: |category| category.display_name(),
        order: |category| category.sort_order(),
    }
}

pub(crate) fn catalog(app: &HaganeApp) -> finder::Catalog {
    finder::Catalog::build(&sources(app))
}

fn adopted(held: core::Pick, target: String) -> Pick {
    let page = held.pane.page();

    let focus = Focus {
        page,
        held: core::Focus {
            realm: realm(page),
            cat: held.cat,
            form: held.form,
            enemy: held.enemy,
            category: None,
            map: held.map,
            stage: held.stage,
            active_mod: Some(target),
            unlocked: false,
        },
    };

    Pick { pane: held.pane, focus, key: held.key, line: held.line, aim: held.aim, file: held.file, label: held.label, path: held.path }
}

fn file_target(app: &HaganeApp, focus: &Focus, target: Option<Target>) -> Option<FileTarget> {
    let Some(Target::FileRow(index)) = target else {
        return None;
    };

    let mount = app.files_state.mount()?;
    let (folder, relative) = app.files_state.entry_at(&app.vault.vfs, index)?;
    let source = app.vault.vfs.root(mount)?.join(&relative);
    let name = relative.file_name()?.to_string_lossy().into_owned();
    let active_mod = focus.active_mod.clone();

    let mod_copy = active_mod.as_deref().and_then(|active| mods::find(&app.vault.vfs, active, Path::new(&name)));

    let game = app.vault.vfs.rooted(architecture::GAME, &name);

    Some(FileTarget {
        source,
        game,
        name,
        mount: mount.to_owned(),
        folder,
        unlocked: focus.unlocked,
        active_mod,
        mod_copy,
    })
}

#[cfg(test)]
mod authoring_tests {
    use super::*;

    fn seed(whole_map: bool) -> Making {
        Making {
            target_mod: Some("MyMod".to_owned()),
            unlocked: false,
            map: 3,
            stage: 0,
            global: 3003,
            whole_map,
            ground: "stageRV003_00.csv".to_owned(),
            data_file: "MapStageDataV_003.csv".to_owned(),
            name_file: "StageName_RV.csv".to_owned(),
            plate: "mapsn003_00_v_en.png".to_owned(),
            map_plate: "mapname003_v_en.png".to_owned(),
            name_key: 3,
            name_cell: 0,
            story: false,
            vanilla: false,
            label: "Stage 00".to_owned(),
        }
    }

    fn names(made: &[(String, Vec<u8>)]) -> Vec<&str> {
        made.iter().map(|(name, _)| name.as_str()).collect()
    }

    // The engine faults on a missing asset, so a new stage has to reach disk with its plate,
    // its cut and its battleground -- never a row in one table and nothing else.
    #[test]
    fn a_new_stage_writes_every_file_it_needs() {
        let made = authored(&seed(false));

        assert_eq!(
            names(&made),
            vec!["mapsn003_00_v_en.png", "mapsn003_00_v_en.imgcut", "stageRV003_00.csv"],
        );

        assert!(made.iter().all(|(_, bytes)| !bytes.is_empty()), "no placeholder may be empty");
    }

    // A map is born holding its first stage: a table with no stage rows is an EmptyFile to
    // codex, and a map with no battleground never reaches the list at all.
    #[test]
    fn a_new_map_writes_its_own_files_and_its_first_stage() {
        let made = authored(&seed(true));

        assert_eq!(
            names(&made),
            vec![
                "mapname003_v_en.png",
                "mapname003_v_en.imgcut",
                "MapStageDataV_003.csv",
                "mapsn003_00_v_en.png",
                "mapsn003_00_v_en.imgcut",
                "stageRV003_00.csv",
            ],
        );
    }
}

#[cfg(test)]
mod mark_tests {
    use super::Mark;

    #[test]
    fn an_entry_is_colored_by_how_the_mod_differs_from_game() {
        assert_eq!(Mark::of(Some("1,2,3"), Some("1,2,3")), Mark::Same);
        assert_eq!(Mark::of(Some("1,2,9"), Some("1,2,3")), Mark::Changed);

        // a talent row or combo the game never had
        assert_eq!(Mark::of(Some("1,2,3"), None), Mark::Added);

        // the mod dropped a row the game still ships
        assert_eq!(Mark::of(None, Some("1,2,3")), Mark::Changed);
        assert_eq!(Mark::of(None::<&str>, None), Mark::Same);
    }
}
