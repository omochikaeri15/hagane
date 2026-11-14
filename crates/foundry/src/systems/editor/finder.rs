use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::Path;

use rustc_hash::{FxHashMap, FxHashSet};

use codex::cat::unit::NyancomboData;
use codex::chapter::map::{RoguelikeEvent, RoguelikeMapSettings, ScoreBonusMap, SpecialRulesMap};
use codex::chapter::stage::Background;
use codex::chapter::treasure::{data_file, GROUPS};
use codex::chapter::Category;
use codex::combat::{Faction, Identity, REGISTRY};
use codex::common;
use codex::graphics::backdrop;

use crate::common::architecture;
use crate::common::formats::json::{self, Record, Value};
use crate::domains::cat::combo as cat_combo;
use crate::domains::cat::files as cat_files;
use crate::domains::cat::scanner::CatEntry;
use crate::domains::enemy::files as enemy_files;
use crate::domains::enemy::scanner::EnemyEntry;
use crate::domains::sandbox::base::{self as sandbox_base, Parts};
use crate::domains::sandbox::{orb, scored};
use crate::domains::settings::EditorMode;
use crate::domains::stage::files as stage_files;
use crate::domains::stage::{GlobalMapId, GlobalStageId};
use crate::domains::utilities::background;

use super::ledger::{Assembly, Ledger, ASSEMBLIES};
use super::records::{self, Book, Named, BOOKS};
use super::sheet::{Sheet, LOGIN_EXTENSION, LOGIN_PREFIX, SHEETS, STORIES, TREASURE_NAMES, TREASURE_RETOLD, TREASURE_SETS};
use super::tables::*;
use super::{
    altar_key, audio, captions, catfruit, combo_effect, copies, drop_ids, dungeon, evolve, figures, filter, gift, glossary_file, group, internal_name, item_label,
    logins, maplayout, orb_named, preset, prose, registry, roster, treasure, unused, visible_name, Art, Pane, Pick, Sources, SEPARATOR,
};

pub struct Words {
    pub title: &'static str,
    pub note: &'static str,
    pub hint: &'static str,
    pub action: &'static str,
    pub empty: &'static str,
}

const RAW_WORDS: Words = Words {
    title: "Edit File",
    note: "Search a file to edit.",
    hint: "Search files...",
    action: "Edit File",
    empty: "No files found",
};

const RESOLVED_WORDS: Words = Words {
    title: "Search Attribute",
    note: "Search an attribute to edit.",
    hint: "Search attributes...",
    action: "Edit Attribute",
    empty: "No attributes found",
};

pub fn words(values: EditorMode) -> &'static Words {
    match values {
        EditorMode::Raw => &RAW_WORDS,
        EditorMode::Resolved => &RESOLVED_WORDS,
    }
}

const TALENT_NAME_PREFIX: &str = "Skill_name_";
const TALENT_NAME_FOLDER: &str = "Talent Name Images";
const TALENT_NAME: &str = "Talent Name";
const TALENT_ORBS: &str = "Talent Orbs";
const TALK_FOLDER: &str = "Event Talks";
const TALK_NAME: &str = "Talk";
const TALK_EXTENSION: &str = "tsv";
const IMAGES: &str = "Images";
const PICTURES: &str = "PNG";
const CUT_LISTS: &str = "IMGCUT";
const PICTURE_EXTENSION: &str = "png";
const CUT_EXTENSION: &str = "imgcut";
const RIG_EXTENSION: &str = "mamodel";
const SOUND_FILES: &str = "Sounds";
const FILES: &str = "Files";
const PLAIN_FILES: &str = "No Extension";
const LOOSE_FAMILY: &str = "0-9";
const SLOT_MACHINES: &str = "Slot Machines";
const SCRATCH_CARDS: &str = "Scratch Cards";
const IMAGE_PARTNERS: [(&str, &str); 15] = [
    ("loginbg_", logins::FOLDER),
    ("equipment_", TALENT_ORBS),
    ("event_talk_", TALK_FOLDER),
    ("gamatoto", GAMATOTO_FOLDER),
    ("gama_", GAMATOTO_FOLDER),
    ("slot_", SLOT_MACHINES),
    ("slotbutton", SLOT_MACHINES),
    ("scratch", SCRATCH_CARDS),
    ("jinja", SHRINE),
    ("omikuji", SHRINE),
    ("gatya_", roster::FOLDER),
    ("nyankoCastle", CAT_BASE),
    ("castleCustom", CAT_BASE),
    ("DecoBaseCustom", CAT_BASE),
    ("BaseCustom", CAT_BASE),
];

fn family(name: &str) -> String {
    let stem: String = name.chars().take_while(|glyph| !glyph.is_ascii_digit() && *glyph != '.').collect();
    let stem = stem.trim_end_matches('_');

    if stem.is_empty() { LOOSE_FAMILY.to_owned() } else { stem.to_owned() }
}

fn rated(bytes: &[u8]) -> FxHashMap<(u32, usize), usize> {
    let mut held = FxHashMap::default();

    for (line, row) in common::scrub(bytes).lines().enumerate() {
        let mut cells = row.split([',', '\t']).map(|cell| cell.trim().parse::<u32>().ok());

        let Some(Some(id)) = cells.next() else {
            break;
        };

        if let Some(form) = cells.next().flatten().and_then(|form| (form as usize).checked_sub(1)) {
            held.insert((id, form), line);
        }
    }

    held
}

fn numbered_stem(stem: &str) -> bool {
    !stem.is_empty() && stem.chars().all(|glyph| glyph.is_ascii_digit())
}

fn sheet_of(name: &str) -> Option<(u32, &str, bool)> {
    let (stem, extension) = name.rsplit_once('.')?;
    let picture = extension.eq_ignore_ascii_case(PICTURE_EXTENSION);

    if !picture && !extension.eq_ignore_ascii_case(CUT_EXTENSION) {
        return None;
    }

    let (set, sheet) = stem.strip_prefix(MAP_SHEET_PREFIX)?.split_once('_')?;

    match set.len() == MAP_SHEET_DIGITS && numbered_stem(set) && numbered_stem(sheet) {
        true => Some((set.parse().ok()?, sheet, picture)),
        false => None,
    }
}

fn backdrop_stem(stem: &str) -> bool {
    match stem.strip_prefix(BACKGROUND_EFFECT_PREFIX) {
        Some(rest) => !rest.is_empty() && rest.split('_').all(numbered_stem),
        None => stem.strip_prefix(BACKGROUND_PREFIX).is_some_and(numbered_stem),
    }
}

fn castle_stem(stem: &str) -> bool {
    let body = stem.strip_prefix(CASTLE_CUT_PREFIX).unwrap_or(stem);

    CASTLE_SETS.iter().filter_map(|(_, _, prefix)| body.strip_prefix(prefix)).any(|rest| match rest.split_once('_') {
        Some((number, language)) => numbered_stem(number) && language.len() == LANGUAGE_TAIL && language.chars().all(|glyph| glyph.is_ascii_lowercase()),
        None => numbered_stem(rest),
    })
}

fn unit_image(name: &str) -> Option<(u32, usize, Art)> {
    let (stem, extension) = name.rsplit_once('.')?;

    if !extension.eq_ignore_ascii_case(PICTURE_EXTENSION) {
        return None;
    }

    let (art, rest) = match (stem.strip_prefix(ICON_PREFIX), stem.strip_prefix(BANNER_PREFIX)) {
        (Some(rest), _) => (Art::DeployIcon, rest),
        (None, Some(rest)) => (Art::Banner, rest),
        (None, None) => return None,
    };

    let mut parts = rest.split('_');
    let id = parts.next().filter(|id| id.len() == UNIT_DIGITS && numbered_stem(id))?;
    let mut form = parts.next()?.chars();
    let letter = form.next()?;
    let frame = form.as_str();
    let language = parts.next();

    let plain = parts.next().is_none() && language.is_none_or(|code| code.len() == LANGUAGE_LETTERS && code.bytes().all(|glyph| glyph.is_ascii_lowercase()));

    let framed = match art {
        Art::DeployIcon => frame.len() == FRAME_DIGITS && numbered_stem(frame),
        _ => frame.is_empty(),
    };

    let slot = (0..figures::FORMS.len()).find(|slot| cat_files::form_letter(*slot) == letter)?;

    match plain && framed {
        true => Some((id.parse().ok()?, slot, art)),
        false => None,
    }
}

fn egg_image(name: &str) -> Option<u32> {
    let (stem, extension) = name.rsplit_once('.')?;

    if !extension.eq_ignore_ascii_case(PICTURE_EXTENSION) {
        return None;
    }

    let rest = stem.strip_prefix(ICON_PREFIX).or_else(|| stem.strip_prefix(BANNER_PREFIX))?;
    let (id, stage) = rest.split_once('_')?;
    let frame = stage.strip_prefix(EGG_MARK)?;

    match id.len() == UNIT_DIGITS && numbered_stem(id) && frame.len() == FRAME_DIGITS && numbered_stem(frame) {
        true => id.parse().ok(),
        false => None,
    }
}

fn rigged(rigs: &[String], sheet: &str) -> bool {
    let Some(stem) = sheet.rsplit_once('.').map(|(stem, _)| stem) else {
        return false;
    };
    let at = rigs.partition_point(|rig| rig.as_str() < stem);

    rigs[at..]
        .iter()
        .map_while(|rig| rig.strip_prefix(stem))
        .any(|rest| rest.starts_with(['.', '_']))
}

const CATS: &str = "Cats";
const ENEMIES: &str = "Enemies";
const STAGES: &str = "Stages";
const OTHER: &str = "Other";
const COMBOS: &str = "Combos";
const COMBO_SETUP: &str = "Setup";
const COMBO_NAME: &str = "Name";

const UNITS: &str = "Units";
const ICON_PREFIX: &str = "uni";
const BANNER_PREFIX: &str = "udi";
const UNIT_DIGITS: usize = 3;
const FRAME_DIGITS: usize = 2;
const LANGUAGE_LETTERS: usize = 2;
const EGGS: &str = "Eggs";
const EGG_NAMES: [&str; 11] =
    ["Dirty", "Traitless", "Red", "Floating", "Dark", "Metal", "Angel", "Alien", "Zombie", "Aku", "Relic"];
const EGG_MARK: &str = "m";
const EGG_STAGES: usize = 2;
const NO_EGG: i32 = -1;

const ROGUELIKE: &str = "Roguelike";
const PRESET_FOLDER_RAW: &str = "fixedlineup";
const PRESET_EVENT: &str = "Event";
const LIST_JOINT: &str = ", ";
const BACKGROUNDS: &str = "Backgrounds";
const UPGRADES: &str = "Power Ups";
const GROUP_NAME: &str = "Group";
const STRENGTHS: &str = "Combo Strengths";
const FILTER_ALL: &str = "Every Effect";
const COMBO_TEXT: &str = "Combo Text";
const MEDAL_TEXT: &str = "Meow Medal Text";
const ORB_TEXT: &str = "Talent Orb Text";
const RANK_TEXT: &str = "Rank Reward Text";
const TREASURE_TEXT: &str = "Text";
const GAMATOTO_TEXT: &str = "Text";
const GAMATOTO_SHEETS: [Sheet; 5] =
    [Sheet::GamatotoAreas, Sheet::GamatotoEventAreas, Sheet::GamatotoConnectors, Sheet::GamatotoObjects, Sheet::GamatotoActions];
const GAMATOTO_CAPTIONS: [captions::Layout; 3] =
    [captions::Layout::GamatotoHelpers, captions::Layout::GamatotoUnlocks, captions::Layout::GamatotoMessages];
const CAT_GOD: &str = "Cat God";
const INTERFACE_TEXT: &str = "Interface Text";
const STORY: &str = "Story";
const SHRINE: &str = "Shrine";
const LOGIN_EVENT: &str = "Event";
const INTERFACE_SHEETS: [Sheet; 6] =
    [Sheet::Warnings, Sheet::PageNames, Sheet::StampMessages, Sheet::CalendarFilters, Sheet::CalendarEvents, Sheet::DailyEvents];
const GOD_SHEETS: [Sheet; 2] = [Sheet::GodName, Sheet::MiracleNames];
const GOD_CAPTIONS: [captions::Layout; 6] = [
    captions::Layout::GodExplanation,
    captions::Layout::GodIntro,
    captions::Layout::GodChatter,
    captions::Layout::GodShortReply,
    captions::Layout::GodPurchaseReply,
    captions::Layout::GodMiracles,
];
const TREASURE_RETOLD_TEXT: &str = "After First Encounter";
const MEDAL_TEXT_RAW: &str = "medalname";
const EVOLVED_TEXT_FORM: usize = 2;
const CAT_BASE: &str = "Cat Base";
const SET_NAME: &str = "Set";
const POINT_KEY: &str = "PointID";
const POINT_FOLDER: &str = "Point Events";
const POINT_FOLDER_RAW: &str = "PointID";
const POINT_EVENT: &str = "Event";
const POINT_BOOKS: [Book; 3] = [Book::PointMaps, Book::PointUnlocks, Book::PointRewards];
const REWARD_FOLDER: &str = "Rewards";
const MISSIONS: &str = "Missions";
const MISSION_BREAK: &str = "<br>";
const EX_LOTTERY: &str = "EX Lottery";
const SHAKES: &str = "Screen Shakes";
const RECIPES: &str = "Recipe Unlocks";
const ITEM_SHOP: &str = "Item Shop";
const ZOMBIE_FOLDER: &str = "Zombie Lottery";
const ZOMBIE_SETTINGS: &str = "Lottery";
const ZOMBIE_TIER: &str = "Tier";
const ZOMBIE_CLEARED: &str = "Cleared";
const EX_MAP_COLUMN: usize = 1;
const LOCK_MAP_COLUMN: usize = 1;
const CPU_FOLDER: &str = "Super CPU";
const MISSION_BOOKS: [Book; 2] = [Book::MissionLimits, Book::GachaMissions];
const REWARD_RESET: &str = "Reward Reset";
const REWARD_FOLDER_RAW: &str = "Rewards";
const REWARD_NAME: &str = "Reward";
const REWARD_UNIT: &str = "Points";
const REWARD_POINTS: &str = "Point";
const PART_TAG: &str = "Part";
const ORB_NAME: &str = "Orb";
const MEDAL_NAMES: &str = "medalname.tsv";
const BACKGROUND_NAME: &str = "Background";
const BACKGROUND_PREFIX: &str = "bg";
const BACKGROUND_EFFECT_PREFIX: &str = "bgEffect_";
const BACKGROUND_STRAYS: &str = "Other Background Images";
const CASTLE_CUT_PREFIX: &str = "castle_";
const CASTLE_STRAYS: &str = "Other Castle Images";
const MAP_SHEET_PREFIX: &str = "map";
const MAP_SHEET: &str = "Sheet";
const MAP_SHEET_SET: &str = "Sheet Set";
const MAP_SHEET_DIGITS: usize = 3;
const LANGUAGE_TAIL: usize = 2;

const ITEM_CATALOG: &str = "Item Catalog";
const ITEM_TEXT: &str = "Item Name & Description";
const ITEM_ICONS: &str = "Item Icons";
const ITEM_ICON_RAW: &str = "gatyaitemD";

const ENEMY_MARK: &str = "e";
const ITEM_TAG: &str = "Item";
const LINE_TAG: &str = "Line";

const KEY_JOINT: char = '\u{1f}';

type Maps = Vec<(u32, Vec<u32>)>;
type Sets = FxHashMap<(Pane, &'static str), FxHashMap<Box<str>, Range<usize>>>;

enum Who {
    Cat { id: u32, form: usize },
    Egg,
    Combo { id: u32, form: usize, line: usize },
    Enemy(u32),
    Map(GlobalMapId),
    Stage(GlobalStageId),
    Item(u32),
    Background(u32),
    Group(u32),
    Part(u32),
    Line(usize),
    Ability(Identity, Faction),
    Global,
    Record(Vec<String>),
}

struct Unit {
    who: Who,
    folder: u32,
    tag: Box<str>,
    named: Box<str>,
    tag_key: Box<str>,
    name_key: Box<str>,
}

struct Folder {
    raw: Box<str>,
    resolved: Box<str>,
    raw_key: Box<str>,
    resolved_key: Box<str>,
    parent: Option<u32>,
    folders: Vec<u32>,
    leaves: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    File,
    Title,
    Unit,
}

#[derive(Clone, Copy)]
struct Row {
    pane: Pane,
    unit: u32,
    file: u32,
    title: u32,
    folder: u32,
    face: Face,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry {
    Folder(u32),
    Leaf(u32),
}

pub struct Line {
    pub entry: Entry,
    pub depth: u16,
    pub open: bool,
}

#[derive(PartialEq)]
pub struct Lead {
    folder: String,
    pane: Pane,
    file: Box<str>,
    unit: Box<str>,
}

#[derive(Default)]
pub struct Filter {
    hits: Option<Vec<bool>>,
    counts: Vec<u32>,
}

impl Filter {
    pub fn searching(&self) -> bool {
        self.hits.is_some()
    }

    pub fn keeps(&self, leaf: u32) -> bool {
        self.hits.as_ref().is_none_or(|hits| hits.get(leaf as usize).copied().unwrap_or_default())
    }

    fn holds(&self, folder: u32) -> bool {
        self.counts.get(folder as usize).is_some_and(|count| *count > 0)
    }
}

#[derive(Default)]
pub struct Catalog {
    units: Vec<Unit>,
    folders: Vec<Folder>,
    roots: Vec<u32>,
    files: Vec<Box<str>>,
    file_keys: Vec<Box<str>>,
    titles: Vec<Box<str>>,
    title_keys: Vec<Box<str>>,
    rows: Vec<Row>,
}

struct Hits {
    files: Vec<bool>,
    titles: Vec<bool>,
    units: Vec<bool>,
    folders: Vec<bool>,
}

impl Hits {
    fn reach(&self, row: &Row) -> bool {
        let held = |hits: &[bool], at: u32| hits.get(at as usize).copied().unwrap_or_default();

        held(&self.files, row.file)
            || held(&self.titles, row.title)
            || held(&self.units, row.unit)
            || held(&self.folders, row.folder)
    }
}

fn medal_names(sources: &Sources<'_>) -> Vec<String> {
    let vfs = &sources.vault.vfs;
    let named = copies(vfs, MEDAL_NAMES).into_iter().next().unwrap_or_else(|| MEDAL_NAMES.to_owned());

    vfs.find(named.as_str()).and_then(|path| vfs.read(&path).ok()).map_or_else(Vec::new, |bytes| {
        common::scrub(&bytes).lines().map(|line| line.split('\t').next().unwrap_or_default().trim().to_owned()).collect()
    })
}

fn orb_name(record: &Record, named: &Named) -> String {
    let lookup = |key: &str| -> Option<&str> {
        let raw = record.field(key).and_then(|field| match &field.value {
            Value::Number(number) => number.parse::<i32>().ok(),
            _ => None,
        })?;

        named.iter().find(|(held, _)| *held == key)?.1.iter().find(|(at, _)| *at == raw).map(|(_, name)| name.as_str())
    };

    let index = record.path.last().map_or("", String::as_str);
    let effect = lookup(ORB_EFFECT).map_or_else(|| format!("{ORB_NAME} {index}"), str::to_owned);

    let mut name = match lookup(ORB_GRADE) {
        Some(grade) => format!("{grade} {effect}"),
        None => effect,
    };

    if let Some(target) = lookup(ORB_TRAIT) {
        name.push_str(&format!(" ({target})"));
    }

    name
}

fn keyed_maps(sources: &Sources<'_>, book: Book) -> FxHashSet<u32> {
    sources.vault.vfs.load(book.base()).map_or_else(FxHashSet::default, |bytes| {
        let text = common::scrub(&bytes);

        json::objects(&text)
            .into_iter()
            .filter(|node| node.path.len() == 2 && node.path.first().map(String::as_str) == book.map_root())
            .filter_map(|node| node.path.get(1)?.parse().ok())
            .collect()
    })
}

fn column_keys(sources: &Sources<'_>, ledger: Ledger, column: usize) -> FxHashSet<u32> {
    sources.vault.vfs.load(ledger.base()).map_or_else(FxHashSet::default, |bytes| {
        common::scrub(&bytes).lines().filter_map(|line| line.split(',').nth(column)?.trim().parse().ok()).collect()
    })
}

fn leading_keys(sources: &Sources<'_>, ledger: Ledger) -> FxHashSet<u32> {
    sources.vault
        .vfs
        .load(ledger.base())
        .map_or_else(FxHashSet::default, |bytes| {
            common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect()
        })
}

fn ruled(sources: &Sources<'_>) -> FxHashSet<u32> {
    let vfs = &sources.vault.vfs;

    vfs.load(Book::SpecialRules.base())
        .and_then(|bytes| SpecialRulesMap::parse(bytes.as_ref()).ok())
        .map_or_else(FxHashSet::default, |rules| rules.entries.keys().copied().collect())
}

fn bonused(sources: &Sources<'_>) -> FxHashSet<u32> {
    let vfs = &sources.vault.vfs;

    vfs.load(Book::ScoreBonus.base())
        .and_then(|bytes| ScoreBonusMap::parse(bytes.as_ref()).ok())
        .map_or_else(FxHashSet::default, |bonuses| bonuses.entries.keys().copied().collect())
}

impl Catalog {
    pub fn build(sources: &Sources<'_>) -> Catalog {
        let mut pictures: Vec<String> = Vec::new();
        let mut cuts: Vec<String> = Vec::new();
        let mut rigs: Vec<String> = Vec::new();
        let mut sounds: Vec<String> = Vec::new();

        for name in sources.vault.vfs.glob("").iter() {
            let lowered = name.to_ascii_lowercase();

            match lowered.rsplit_once('.').map(|(_, extension)| extension) {
                Some(PICTURE_EXTENSION) => pictures.push(name.to_string()),
                Some(CUT_EXTENSION) => cuts.push(name.to_string()),
                Some(RIG_EXTENSION) => rigs.push(name.to_string()),
                Some(extension) if audio::EXTENSIONS.contains(&extension) => sounds.push(name.to_string()),
                _ => {}
            }
        }

        rigs.sort_unstable();

        let mut builder = Builder {
            sources,
            catalog: Catalog::default(),
            files_at: FxHashMap::default(),
            titles_at: FxHashMap::default(),
            sets: FxHashMap::default(),
            arena: Vec::new(),
            ruled: ruled(sources),
            bonused: bonused(sources),
            pointed: scored::score_stage_maps(&sources.vault.vfs),
            drawn: leading_keys(sources, Ledger::StageDraws),
            checkpointed: leading_keys(sources, Ledger::Checkpoints),
            limited: leading_keys(sources, Ledger::StageLimits),
            ex_drawn: column_keys(sources, Ledger::ExDraws, EX_MAP_COLUMN),
            rewarded: keyed_maps(sources, Book::ClearRewards),
            displayed: keyed_maps(sources, Book::EventDisplay),
            cat_base: None,
            cat_homes: FxHashMap::default(),
            form_homes: FxHashMap::default(),
            homes: FxHashMap::default(),
            galleries: FxHashMap::default(),
            cpu_locked: column_keys(sources, Ledger::CpuLocks, LOCK_MAP_COLUMN),
            altars: sources.vault.vfs.load(ALTAR_TABLE).map_or_else(FxHashSet::default, |bytes| {
                common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect()
            }),
            pictures,
            cuts,
            rigs,
            sounds,
        };

        let cats = builder.folder(None, CATS.to_owned(), CATS.to_owned());
        let units = builder.folder(Some(cats), UNITS.to_owned(), UNITS.to_owned());
        builder.cats(units);
        builder.unit_images(units);
        builder.numbered(cats, SPOTLIGHT_TABLE, SPOTLIGHT_FOLDER, SPOTLIGHT_NAME, figures::Subject::GuideSpotlights, 0);

        let eggs = builder.folder(Some(cats), EGGS.to_owned(), EGGS.to_owned());
        builder.eggs(eggs);
        builder.talent_names(cats);
        builder.abilities(cats, Faction::Cat);
        builder.book(cats, Book::Orbs);
        builder.grades(cats);
        builder.orb_traits(cats);
        builder.book(cats, Book::Catseyes);
        builder.strengths(cats);
        builder.filters(cats);
        builder.keyed(cats, COMBO_CATEGORY_TABLE, COMBO_CATEGORY_FOLDER, COMBO_CATEGORY_NAME, figures::Subject::ComboCategories);
        builder.combo_text(cats);
        builder.captions(cats, captions::Layout::HiddenHint);
        builder.text_folder(cats, ORB_TEXT, &[Sheet::OrbNames, Sheet::OrbDescriptions, Sheet::TraitNames, Sheet::TraitDescriptions]);

        let enemies = builder.folder(None, ENEMIES.to_owned(), ENEMIES.to_owned());
        let units = builder.folder(Some(enemies), UNITS.to_owned(), UNITS.to_owned());
        builder.enemies(units);
        builder.abilities(enemies, Faction::Enemy);
        builder.captions(enemies, captions::Layout::UnseenEnemy);

        let stages = builder.folder(None, STAGES.to_owned(), STAGES.to_owned());
        builder.stages(stages);
        builder.castles(stages);
        builder.backgrounds(stages);
        builder.map_layouts(stages);
        builder.map_sheets(stages);
        builder.quest(stages);
        builder.realms(stages);
        builder.keyed(stages, DEMON_CASTLE_TABLE, DEMON_CASTLE_FOLDER, DEMON_CASTLE_NAME, figures::Subject::DemonCastles);
        builder.labyrinth(stages);
        builder.enigma(stages);
        builder.map_markers(stages);
        builder.numbered(stages, FILTER_MAP_TABLE, FILTER_MAP_FOLDER, FILTER_MAP_NAME, figures::Subject::FilterMaps, 1);
        builder.numbered(stages, REFUND_TABLE, REFUND_FOLDER, REFUND_NAME, figures::Subject::EnergyRefunds, 0);
        builder.numbered(stages, STAGE_SHORTCUT_TABLE, STAGE_SHORTCUT_FOLDER, STAGE_SHORTCUT_NAME, figures::Subject::StageShortcuts, 0);
        builder.captions(stages, captions::Layout::StageLimits);
        builder.captions(stages, captions::Layout::FirstTips);
        builder.captions(stages, captions::Layout::ClearGifts);
        builder.captions(stages, captions::Layout::StageHints);

        let other = builder.folder(None, OTHER.to_owned(), OTHER.to_owned());
        builder.roguelike(other);
        builder.upgrades(other);
        builder.base(other);
        builder.ex_lottery(other);
        builder.shakes(other);
        builder.recipes(other);
        builder.shop(other);
        builder.gifts(other);
        builder.gacha(other);
        builder.fanfares(other);
        builder.rank_text(other);
        builder.zombies(other);
        builder.cpu(other);
        builder.gamatoto(other);
        builder.god(other);
        builder.interface(other);
        builder.story(other);
        builder.shrine(other);
        builder.shrine_tables(other);
        builder.catfruit(other);
        builder.sounds(other);
        builder.sound_files(other);
        builder.captions(other, captions::Layout::CollabSends);
        builder.chests(other);
        builder.prizes(other);
        builder.reels(other);
        builder.keyed(other, TIP_SETTING_TABLE, TIP_SETTING_FOLDER, TIP_SETTING_NAME, figures::Subject::TipSettings);
        builder.keyed(other, MENU_TEXT_TABLE, MENU_TEXT_FOLDER, MENU_TEXT_NAME, figures::Subject::MenuTextSettings);
        builder.numbered(other, EXCLUSION_TABLE, EXCLUSION_FOLDER, EXCLUSION_NAME, figures::Subject::LineupExclusions, 0);
        builder.numbered(other, COUNTER_TABLE, COUNTER_FOLDER, COUNTER_NAME, figures::Subject::LineupCounters, 0);
        builder.captions(other, captions::Layout::Vibrations);
        builder.talks(other);
        builder.numbered(other, ABILITY_ICON_TABLE, ABILITY_ICON_FOLDER, ABILITY_ICON_NAME, figures::Subject::AbilityIcons, 0);
        builder.numbered(other, TRAIT_ICON_TABLE, TRAIT_ICON_FOLDER, TRAIT_ICON_NAME, figures::Subject::TraitIcons, 0);
        builder.numbered(other, BAND_TABLE, BAND_FOLDER, BAND_NAME, figures::Subject::LineupBands, 0);
        builder.numbered(other, RANK_GROUP_TABLE, RANK_GROUP_FOLDER, RANK_GROUP_NAME, figures::Subject::LineupRankGroups, 0);
        builder.numbered(other, RECOMMEND_TABLE, RECOMMEND_FOLDER, RECOMMEND_NAME, figures::Subject::RecommendedUpgrades, 0);
        builder.keyed(other, LEVEL_UP_TABLE, LEVEL_UP_FOLDER, LEVEL_UP_NAME, figures::Subject::LevelUpOffers);
        builder.numbered(other, ORB_CHOICE_TABLE, ORB_CHOICE_FOLDER, ORB_CHOICE_NAME, figures::Subject::OrbFilterChoices, 0);
        builder.keyed(other, ORB_OPTION_TABLE, ORB_OPTION_FOLDER, ORB_OPTION_NAME, figures::Subject::OrbOptions);
        builder.unused(other);
        builder.numbered(other, STAMP_TABLE, STAMP_FOLDER, STAMP_DAY, figures::Subject::Stamps, 0);
        builder.numbered(other, SLOT_TABLE, SLOT_FOLDER, SLOT_NAME, figures::Subject::SlotUnlocks, 1);
        builder.keyed(other, CLUB_TABLE, CLUB_FOLDER, CLUB_NAME, figures::Subject::Club);
        builder.treasures(other);
        builder.groups(other);
        builder.book(other, Book::RuleBans);
        builder.book(other, Book::Medals);
        builder.medal_text(other);
        builder.book(other, Book::MapConditions);
        builder.points(other);
        builder.missions(other);
        builder.book(other, Book::GachaBonus);
        builder.book(other, Book::LoginStamps);
        builder.login_events(other);
        builder.login_messages(other);
        builder.book(other, Book::UnlockPopups);
        builder.keyed(other, POPUP_TRIGGER_TABLE, POPUP_TRIGGER_FOLDER, POPUP_TRIGGER_NAME, figures::Subject::PopupTriggers);
        builder.whole(other, Book::RemoteConfig);
        builder.items(other);
        builder.sheets(other);
        builder.images(other);
        builder.files(other);

        builder.catalog
    }

    pub fn label(&self, entry: Entry, values: EditorMode) -> &str {
        match entry {
            Entry::Folder(id) => self.folder_label(id, values),
            Entry::Leaf(at) => self.leaf_label(at, values).unwrap_or_default(),
        }
    }

    fn folder_label(&self, id: u32, values: EditorMode) -> &str {
        self.folders.get(id as usize).map_or("", |folder| match values {
            EditorMode::Raw => &folder.raw,
            EditorMode::Resolved => &folder.resolved,
        })
    }

    fn leaf_label(&self, at: u32, values: EditorMode) -> Option<&str> {
        let row = self.rows.get(at as usize)?;
        let unit = self.units.get(row.unit as usize)?;

        match (row.face, values) {
            (Face::File, EditorMode::Raw) => self.files.get(row.file as usize).map(AsRef::as_ref),
            (Face::File, EditorMode::Resolved) | (Face::Title, _) => self.titles.get(row.title as usize).map(AsRef::as_ref),
            (Face::Unit, EditorMode::Raw) => Some(&unit.tag),
            (Face::Unit, EditorMode::Resolved) => Some(&unit.named),
        }
    }

    pub fn filter(&self, query: &str) -> Filter {
        let lowered = query.to_lowercase();

        let tokens: Vec<&str> = lowered
            .split_whitespace()
            .map(|token| token.trim_matches(['(', ')']))
            .filter(|token| !token.is_empty())
            .collect();

        let hits: Option<Vec<bool>> = (!tokens.is_empty()).then(|| {
            let found: Vec<Hits> = tokens.iter().map(|token| self.hits(token)).collect();

            self.rows.iter().map(|row| found.iter().all(|hit| hit.reach(row))).collect()
        });

        let mut counts = vec![0u32; self.folders.len()];

        for (at, row) in self.rows.iter().enumerate() {
            let kept = hits.as_ref().is_none_or(|hits| hits.get(at).copied().unwrap_or_default());

            if let Some(count) = counts.get_mut(row.folder as usize)
                && kept
            {
                *count += 1;
            }
        }

        for at in (0..self.folders.len()).rev() {
            let Some(parent) = self.folders[at].parent else {
                continue;
            };

            let held = counts[at];

            if let Some(count) = counts.get_mut(parent as usize) {
                *count += held;
            }
        }

        Filter { hits, counts }
    }

    fn hits(&self, token: &str) -> Hits {
        let mut folders = vec![false; self.folders.len()];

        for (at, folder) in self.folders.iter().enumerate() {
            let inherited = folder.parent.is_some_and(|parent| folders.get(parent as usize).copied().unwrap_or_default());

            folders[at] = inherited || folder.raw_key.contains(token) || folder.resolved_key.contains(token);
        }

        Hits {
            files: self.file_keys.iter().map(|key| key.contains(token)).collect(),
            titles: self.title_keys.iter().map(|key| key.contains(token)).collect(),
            units: self.units.iter().map(|unit| unit.tag_key.contains(token) || unit.name_key.contains(token)).collect(),
            folders,
        }
    }

    pub fn lines(&self, filter: &Filter, open: impl Fn(u32) -> bool) -> Vec<Line> {
        let mut lines = Vec::new();

        for root in &self.roots {
            self.walk(*root, 0, filter, &open, &mut lines);
        }

        lines
    }

    fn walk(&self, id: u32, depth: u16, filter: &Filter, open: &impl Fn(u32) -> bool, lines: &mut Vec<Line>) {
        if !filter.holds(id) {
            return;
        }

        let Some(folder) = self.folders.get(id as usize) else {
            return;
        };

        let opened = open(id);

        lines.push(Line { entry: Entry::Folder(id), depth, open: opened });

        if !opened {
            return;
        }

        for child in &folder.folders {
            self.walk(*child, depth + 1, filter, open, lines);
        }

        for leaf in &folder.leaves {
            if !filter.keeps(*leaf) {
                continue;
            }

            lines.push(Line { entry: Entry::Leaf(*leaf), depth: depth + 1, open: false });
        }
    }

    pub fn folder_key(&self, id: u32) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut at = Some(id);

        while let Some(held) = at.and_then(|id| self.folders.get(id as usize)) {
            parts.push(&held.raw);
            at = held.parent;
        }

        if parts.is_empty() {
            return None;
        }

        parts.reverse();

        Some(parts.join(&KEY_JOINT.to_string()))
    }

    pub fn folder_ids(&self) -> FxHashMap<String, u32> {
        (0..u32::try_from(self.folders.len()).unwrap_or(u32::MAX))
            .filter_map(|id| Some((self.folder_key(id)?, id)))
            .collect()
    }

    pub fn lead(&self, leaf: u32) -> Option<Lead> {
        let row = self.rows.get(leaf as usize)?;

        Some(Lead {
            folder: self.folder_key(row.folder)?,
            pane: row.pane,
            file: self.files.get(row.file as usize)?.clone(),
            unit: self.units.get(row.unit as usize)?.tag_key.clone(),
        })
    }

    pub fn follow(&self, lead: &Lead, folders: &FxHashMap<String, u32>) -> Option<u32> {
        let folder = self.folders.get(*folders.get(&lead.folder)? as usize)?;

        folder.leaves.iter().copied().find(|leaf| self.lead(*leaf).as_ref() == Some(lead))
    }

    pub fn pick(&self, row: u32) -> Option<Pick> {
        let row = self.rows.get(row as usize)?;
        let unit = self.units.get(row.unit as usize)?;
        let file = self.files.get(row.file as usize)?;

        let mut pick = Pick {
            pane: row.pane,
            cat: None,
            form: 0,
            enemy: None,
            map: None,
            stage: None,
            key: None,
            line: None,
            aim: None,
            file: file.to_string(),
            label: [unit.named.as_ref(), file.as_ref()].join(SEPARATOR),
            path: Vec::new(),
        };

        match &unit.who {
            Who::Cat { id, form } => {
                pick.cat = Some(*id);
                pick.form = *form;
            }
            Who::Combo { id, form, line } => {
                pick.cat = Some(*id);
                pick.form = *form;
                pick.line = Some(*line);
            }
            Who::Enemy(id) => pick.enemy = Some(*id),
            Who::Map(map) => pick.map = Some(map.clone()),
            Who::Stage(stage) => {
                pick.map = Some(GlobalMapId { category: stage.category.clone(), map: stage.map });
                pick.stage = Some(stage.clone());
            }
            Who::Item(id) | Who::Background(id) | Who::Group(id) | Who::Part(id) => pick.key = Some(*id),
            Who::Line(line) => pick.line = Some(*line),
            Who::Egg | Who::Global => {}
            Who::Record(held) => pick.path.clone_from(held),
            Who::Ability(identity, faction) => pick.aim = Some((*identity, *faction)),
        }

        Some(pick)
    }
}

#[derive(Clone, Copy)]
enum Source {
    Single,
    Variants,
    Languages,
    Exception,
    Image,
}

struct Builder<'a> {
    sources: &'a Sources<'a>,
    catalog: Catalog,
    files_at: FxHashMap<Box<str>, u32>,
    titles_at: FxHashMap<Box<str>, u32>,
    sets: Sets,
    arena: Vec<(u32, u32)>,
    ruled: FxHashSet<u32>,
    bonused: FxHashSet<u32>,
    pointed: BTreeSet<i32>,
    drawn: FxHashSet<u32>,
    checkpointed: FxHashSet<u32>,
    limited: FxHashSet<u32>,
    ex_drawn: FxHashSet<u32>,
    rewarded: FxHashSet<u32>,
    displayed: FxHashSet<u32>,
    cat_base: Option<u32>,
    cat_homes: FxHashMap<u32, u32>,
    form_homes: FxHashMap<(u32, usize), u32>,
    homes: FxHashMap<&'static str, u32>,
    galleries: FxHashMap<(&'static str, &'static str), u32>,
    cpu_locked: FxHashSet<u32>,
    altars: FxHashSet<u32>,
    pictures: Vec<String>,
    cuts: Vec<String>,
    rigs: Vec<String>,
    sounds: Vec<String>,
}

impl Builder<'_> {
    fn cats(&mut self, root: u32) {
        let sources = self.sources;
        let mut cats: Vec<&CatEntry> = sources.cats.iter().collect();
        let slotted = sources.vault.vds.cats.orb_slots(&sources.vault.vfs);
        let combos = sources.vault.vds.cats.combos(&sources.vault.vfs);
        let names = sources.vault.vds.cats.combo_names(&sources.vault.vfs);

        cats.sort_by_key(|cat| cat.id);

        let ratings = rated(sources.vault.vfs.load(RATING_TABLE).as_deref().unwrap_or_default());

        for cat in cats {
            let stats = cat_files::stats_file(cat.id);
            let explanation = cat_files::explanation_file(cat.id);
            let talented = cat.talent_data.is_some();
            let orbed = talented || slotted.contains_key(&cat.id);
            let parent = self.folder(Some(root), format!("{:03}", cat.id), cat.display_name(0));

            self.cat_homes.insert(cat.id, parent);

            for (form, held) in cat.forms.iter().enumerate() {
                if !held {
                    continue;
                }

                let tag = format!("{:03}_{}", cat.id, cat_files::form_letter(form));
                let folder = self.folder(Some(parent), tag.clone(), cat.display_name(form));
                let unit = self.unit(Who::Cat { id: cat.id, form }, folder, tag, cat.display_name(form));

                self.form_homes.insert((cat.id, form), unit);
                self.rows(unit, Pane::Figures(figures::Subject::Cat), &stats, Source::Single);
                self.rows(unit, Pane::Prose(prose::Subject::Explanation), &explanation, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::Buy), cat_files::UNIT_BUY, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::Curve), cat_files::UNIT_LEVEL, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::ExpCurve), EXP_TABLE, Source::Variants);
                self.rows(unit, Pane::Captions(captions::Layout::UnlockHints), captions::Layout::UnlockHints.base(), Source::Languages);
                self.rows(unit, Pane::Figures(figures::Subject::GuideEntries), GUIDE_TABLE, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::UnitLimits), UNIT_LIMIT_TABLE, Source::Variants);

                if let Some(line) = ratings.get(&(cat.id, form)) {
                    let rating = self.unit(Who::Line(*line), folder, format!("{LINE_TAG} {line}"), cat.display_name(form));

                    self.rows(rating, Pane::Figures(figures::Subject::LineupRatings), RATING_TABLE, Source::Variants);
                }

                if form >= EVOLVED_TEXT_FORM {
                    self.rows(unit, Pane::Evolve, evolve::TABLE, Source::Variants);
                }
                self.rows(unit, Pane::Figures(figures::Subject::Talents), cat_files::SKILL_ACQUISITION, Source::Variants);

                if orbed {
                    self.rows(unit, Pane::Figures(figures::Subject::OrbSlots), ORB_SLOT_TABLE, Source::Variants);
                }

                if talented {
                    self.rows(unit, Pane::Figures(figures::Subject::Costs), cat_files::SKILL_LEVEL, Source::Variants);
                    self.rows(unit, Pane::Prose(prose::Subject::TalentText), cat_files::SKILL_DESCRIPTIONS, Source::Exception);
                }

                self.combos(folder, (cat.id, form), &combos, &names);

                let banner = cat_files::banner_file(cat.id, form, cat.egg_ids.unwrap_or(NO_EGGS));

                if let Some(icon) = cat.deploy_icon_paths.get(form).and_then(Option::as_deref).and_then(Path::file_name) {
                    self.rows(unit, Pane::Artwork(Art::DeployIcon), &icon.to_string_lossy(), Source::Image);
                }

                self.rows(unit, Pane::Artwork(Art::Banner), &banner, Source::Image);

                if form == 0 {
                    let gacha = cat_files::gacha_file(cat.id, cat.egg_ids.unwrap_or(NO_EGGS));

                    self.rows(unit, Pane::Artwork(Art::Gacha), &gacha, Source::Image);
                }
            }
        }
    }

    fn combos(&mut self, parent: u32, (id, form): (u32, usize), rows: &[NyancomboData], names: &[Option<String>]) {
        let group = self.folder(Some(parent), COMBOS.to_owned(), COMBOS.to_owned());

        for line in cat_combo::combo_lines(self.sources.vault, id, form) {
            let key = figures::combo_key(rows, line);
            let title = figures::titled(rows, names, line);
            let folder = self.folder(Some(group), key.clone(), title.clone());
            let unit = self.unit(Who::Combo { id, form, line }, folder, key, title);

            self.headed(unit, Pane::Prose(prose::Subject::ComboName), cat_files::NYANCOMBO_NAME, COMBO_NAME);
            self.headed(unit, Pane::Figures(figures::Subject::Combo), cat_files::NYANCOMBO_DATA, COMBO_SETUP);
        }

        let tag = format!("{id:03}_{}", cat_files::form_letter(form));
        let unit = self.unit(Who::Cat { id, form }, group, tag, figures::NEW_COMBO.to_owned());

        self.fresh(unit, Pane::Figures(figures::Subject::Combo), cat_files::NYANCOMBO_DATA, figures::NEW_COMBO);
    }

    fn unit_images(&mut self, root: u32) {
        let vfs = &self.sources.vault.vfs;

        let mut loose: Vec<(u32, usize, Art, String)> = [ICON_PREFIX, BANNER_PREFIX]
            .into_iter()
            .flat_map(|prefix| vfs.glob(prefix))
            .filter(|name| !self.files_at.contains_key(name.as_ref()))
            .filter_map(|name| unit_image(&name).map(|(id, form, art)| (id, form, art, name.to_string())))
            .collect();

        loose.sort_unstable_by(|left, right| (left.0, left.1, &left.3).cmp(&(right.0, right.1, &right.3)));

        for (id, form, art, name) in loose {
            if self.files_at.contains_key(name.as_str()) {
                continue;
            }

            let base = internal_name(self.sources, &name);
            let source = match base == name {
                true => Source::Single,
                false => Source::Image,
            };

            let unit = match self.form_homes.get(&(id, form)).copied() {
                Some(unit) => unit,
                None => {
                    let home = match self.cat_homes.get(&id).copied() {
                        Some(home) => home,
                        None => {
                            let home = self.folder(Some(root), format!("{id:03}"), format!("{id:03}"));

                            self.cat_homes.insert(id, home);
                            home
                        }
                    };

                    let tag = format!("{id:03}_{}", cat_files::form_letter(form));
                    let titled = figures::FORMS.get(form).copied().unwrap_or_default().to_owned();
                    let folder = self.folder(Some(home), tag.clone(), titled.clone());
                    let unit = self.unit(Who::Global, folder, tag, titled);

                    self.form_homes.insert((id, form), unit);
                    unit
                }
            };

            self.rows(unit, Pane::Artwork(art), &base, source);
        }
    }

    fn eggs(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let mut used: Vec<u32> = self
            .sources
            .cats
            .iter()
            .filter_map(|cat| cat.egg_ids)
            .flat_map(|(norm, evol)| [norm, evol])
            .filter_map(|egg| u32::try_from(egg).ok())
            .chain([ICON_PREFIX, BANNER_PREFIX].into_iter().flat_map(|prefix| vfs.glob(prefix)).filter_map(|name| egg_image(&name)))
            .collect();

        used.sort_unstable();
        used.dedup();

        for id in used {
            let Ok(raw) = i32::try_from(id) else {
                continue;
            };

            let tag = format!("{id:03}_{EGG_MARK}");
            let name = EGG_NAMES.get(id as usize).map_or_else(|| format!("{id:03}"), |named| (*named).to_owned());
            let folder = self.folder(Some(parent), tag, name.clone());

            for stage in 0..EGG_STAGES {
                let eggs = match stage {
                    0 => (raw, NO_EGG),
                    _ => (NO_EGG, raw),
                };

                let held = format!("{id:03}_{EGG_MARK}{stage:02}");
                let titled = figures::FORMS.get(stage).copied().unwrap_or_default().to_owned();
                let sub = self.folder(Some(folder), held.clone(), titled);
                let unit = self.unit(Who::Egg, sub, held, name.clone());

                self.rows(unit, Pane::Artwork(Art::DeployIcon), &cat_files::icon_file(0, stage, eggs), Source::Image);
                self.rows(unit, Pane::Artwork(Art::Banner), &cat_files::banner_file(0, stage, eggs), Source::Image);

                if stage == 0 {
                    self.rows(unit, Pane::Artwork(Art::Gacha), &cat_files::gacha_file(0, eggs), Source::Image);
                }
            }
        }
    }

    fn enemies(&mut self, root: u32) {
        let mut enemies: Vec<&EnemyEntry> = self.sources.enemies.iter().collect();
        let guided: FxHashSet<u32> = self.sources.vault.vfs.load(ENEMY_GUIDE_TABLE).map_or_else(FxHashSet::default, |bytes| {
            common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect()
        });

        enemies.sort_by_key(|enemy| enemy.id);

        for enemy in enemies {
            let tag = format!("{:03}_{ENEMY_MARK}", enemy.id);
            let folder = self.folder(Some(root), tag.clone(), enemy.display_name());
            let unit = self.unit(Who::Enemy(enemy.id), folder, tag, enemy.display_name());

            self.rows(unit, Pane::Figures(figures::Subject::Enemy), enemy_files::STATS, Source::Single);
            self.rows(unit, Pane::Prose(prose::Subject::EnemyName), enemy_files::NAMES, Source::Exception);
            self.rows(unit, Pane::Prose(prose::Subject::EnemyDescription), enemy_files::PICTURE_BOOK, Source::Variants);

            if guided.contains(&enemy.id) {
                self.rows(unit, Pane::Figures(figures::Subject::EnemyGuide), ENEMY_GUIDE_TABLE, Source::Variants);
            }

            if let Some(icon) = enemy.icon_path.as_deref().and_then(Path::file_name) {
                self.rows(unit, Pane::Artwork(Art::EnemyIcon), &icon.to_string_lossy(), Source::Image);
            }
        }
    }

    fn stages(&mut self, root: u32) {
        let sources = self.sources;
        let registry = sources.registry;

        let mut held: FxHashMap<GlobalMapId, Vec<u32>> = FxHashMap::default();

        for key in registry.maps.keys() {
            held.entry(key.clone()).or_default();
        }

        for key in registry.stages.keys() {
            held.entry(GlobalMapId { category: key.category.clone(), map: key.map }).or_default().push(key.stage);
        }

        let mut grouped: FxHashMap<Category, Maps> = FxHashMap::default();

        for (key, stages) in held {
            grouped.entry(key.category).or_default().push((key.map, stages));
        }

        let mut categories: Vec<(Category, Maps)> = grouped.into_iter().collect();

        categories.sort_by(|(left, _), (right, _)| {
            ((sources.order)(left), left.map_prefix()).cmp(&((sources.order)(right), right.map_prefix()))
        });

        for (category, mut maps) in categories {
            let prefix = category.map_prefix();
            let raw = if prefix.is_empty() { (sources.category)(&category).to_owned() } else { prefix.clone() };
            let folder = self.folder(Some(root), raw, (sources.category)(&category).to_owned());

            maps.sort_unstable_by_key(|(map, _)| *map);

            for (map, stages) in maps {
                self.map(folder, GlobalMapId { category: category.clone(), map }, &prefix, stages);
            }
        }
    }

    fn map(&mut self, parent: u32, map: GlobalMapId, prefix: &str, mut stages: Vec<u32>) {
        let sources = self.sources;
        let registry = sources.registry;
        let address = registry.addresses.get(&map);
        let global = address.and_then(|held| held.global);
        let options = sources.vault.vds.stages.map_options(&sources.vault.vfs);
        let formations = sources.vault.vds.stages.fixed_formations(&sources.vault.vfs);
        let ruled = global.is_some_and(|id| self.ruled.contains(&id));
        let bonused = global.is_some_and(|id| self.bonused.contains(&id));
        let pointed = global.and_then(|id| i32::try_from(id).ok()).is_some_and(|id| self.pointed.contains(&id));
        let rated = global.is_some_and(|id| sources.vault.vds.stages.difficulties(&sources.vault.vfs).contains_key(&id));
        let invades = global.is_some_and(|id| sources.vault.vds.stages.ex_options(&sources.vault.vfs).contains_key(&id));

        let map_tag = match prefix.is_empty() {
            true => format!("{:03}", map.map),
            false => format!("{prefix}_{:03}", map.map),
        };

        let chapter = registry
            .maps
            .get(&map)
            .map(|found| found.name.trim())
            .filter(|name| !name.is_empty())
            .map_or_else(|| map_tag.clone(), str::to_owned);

        let dropping = registry.maps.get(&map).is_some_and(|found| found.drop_items.is_some());
        let folder = self.folder(Some(parent), map_tag.clone(), chapter.clone());
        let unit = self.unit(Who::Map(map.clone()), folder, map_tag.clone(), chapter.clone());

        if global.is_some() {
            self.rows(unit, Pane::Prose(prose::Subject::MapName), stage_files::MAP_NAME, Source::Exception);
        }

        if global.is_some() && dropping {
            self.rows(unit, Pane::Figures(figures::Subject::MapDrops), stage_files::DROP_ITEM, Source::Variants);
        }

        if global.is_some_and(|id| options.contains_key(&id)) {
            self.rows(unit, Pane::Figures(figures::Subject::MapOption), stage_files::MAP_OPTION, Source::Variants);
        }

        if ruled {
            let rules = Book::SpecialRules;

            self.rows(unit, Pane::Records(rules), rules.base(), Source::Variants);
        }

        if bonused {
            let bonuses = Book::ScoreBonus;

            self.rows(unit, Pane::Records(bonuses), bonuses.base(), Source::Variants);
        }

        if rated {
            self.rows(unit, Pane::Figures(figures::Subject::Difficulty), DIFFICULTY_TABLE, Source::Variants);
        }

        if invades {
            self.rows(unit, Pane::Figures(figures::Subject::ExInvasion), EX_OPTION_TABLE, Source::Variants);
        }

        if global.is_some_and(|id| self.displayed.contains(&id)) {
            let display = Book::EventDisplay;

            self.rows(unit, Pane::Records(display), display.base(), Source::Variants);
        }

        if global.is_some_and(|id| self.rewarded.contains(&id)) {
            let rewards = Book::ClearRewards;

            self.rows(unit, Pane::Records(rewards), rewards.base(), Source::Variants);
        }

        if pointed {
            let points = Book::PointRules;

            self.rows(unit, Pane::Records(points), points.base(), Source::Variants);
        }

        if global.is_some() {
            let restrictions = Ledger::StageOption;

            self.rows(unit, Pane::Ledger(restrictions), restrictions.base(), Source::Variants);

            let assignments = Ledger::FixedFormation;

            self.rows(unit, Pane::Ledger(assignments), assignments.base(), Source::Variants);
        }

        if global.is_some_and(|id| self.cpu_locked.contains(&id)) {
            let locks = Ledger::CpuLocks;

            self.rows(unit, Pane::Ledger(locks), locks.base(), Source::Variants);
        }

        if global.is_some_and(|id| self.ex_drawn.contains(&id)) {
            let draws = Ledger::ExDraws;

            self.rows(unit, Pane::Ledger(draws), draws.base(), Source::Variants);
        }

        if global.is_some_and(|id| self.limited.contains(&id)) {
            let limits = Ledger::StageLimits;

            self.rows(unit, Pane::Ledger(limits), limits.base(), Source::Variants);
        }

        if global.is_some_and(|id| self.checkpointed.contains(&id)) {
            let checkpoints = Ledger::Checkpoints;

            self.rows(unit, Pane::Ledger(checkpoints), checkpoints.base(), Source::Variants);
        }

        if global.is_some_and(|id| self.drawn.contains(&id)) {
            let draws = Ledger::StageDraws;

            self.rows(unit, Pane::Ledger(draws), draws.base(), Source::Variants);
        }

        let image = map.category.image_prefix();

        self.rows(unit, Pane::Artwork(Art::MapBanner), &stage_files::map_banner_file(map.map, &image), Source::Variants);

        stages.sort_unstable();

        for stage in stages {
            let key = GlobalStageId { category: map.category.clone(), map: map.map, stage };
            let altared = altar_key(sources, &key).is_some_and(|held| self.altars.contains(&held));
            let entry = registry.stages.get(&key);

            let named = entry
                .map(|found| found.name.trim())
                .filter(|name| !name.is_empty())
                .map_or_else(|| format!("{stage:02}"), str::to_owned);

            let tag = format!("{map_tag}_{stage:02}");
            let name = [chapter.as_str(), named.as_str()].join(SEPARATOR);
            let ground = registry.grounds.get(&key).map(|file| file.to_string());
            let drops = entry.is_some_and(|found| drop_ids(found).any(|id| sources.drops.contains_key(&id)));

            let at = self.folder(Some(folder), tag.clone(), named);
            let unit = self.unit(Who::Stage(key), at, tag, name);

            if let Some(file) = address.and_then(|held| held.data_file.as_deref()) {
                self.rows(unit, Pane::Figures(figures::Subject::MapStage), file, Source::Variants);
            }

            if let Some(file) = ground {
                self.rows(unit, Pane::Ground, &file, Source::Single);
            }

            if let Some(file) = address.and_then(|held| held.name_file.as_deref()) {
                self.rows(unit, Pane::Prose(prose::Subject::StageName), file, Source::Variants);
            }

            if drops {
                self.rows(unit, Pane::Figures(figures::Subject::DropChara), stage_files::DROP_CHARA, Source::Variants);
            }

            if let Some(global) = global {
                let mut lineups: Vec<(u8, &str)> = formations
                    .iter()
                    .filter(|((held, _, at), _)| *held == global && *at == stage)
                    .map(|((_, level, _), entry)| (*level, entry.preset_file_name.as_str()))
                    .collect();

                lineups.sort_unstable();

                for (_, file) in lineups {
                    self.rows(unit, Pane::Preset, file, Source::Single);
                }
            }

            if altared {
                self.rows(unit, Pane::Figures(figures::Subject::Altar), ALTAR_TABLE, Source::Variants);
            }

            let plate = stage_files::stage_banner_file(&map.category, map.map, stage, &image);

            self.rows(unit, Pane::Artwork(Art::StageBanner), &plate, Source::Variants);
        }
    }

    fn roguelike(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let mut root: Option<u32> = None;

        for book in BOOKS {
            let Some(text) = vfs
                .find(book.base())
                .and_then(|path| vfs.read(&path).ok())
                .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
            else {
                continue;
            };

            let found: Vec<Record> = json::records(&text).into_iter().filter(|record| record.leaf).collect();

            if found.is_empty() {
                continue;
            }

            let holder = match root {
                Some(held) => held,
                None => {
                    let made = self.folder(Some(parent), ROGUELIKE.to_owned(), ROGUELIKE.to_owned());
                    root = Some(made);

                    made
                }
            };

            let folder = self.folder(Some(holder), book.base().to_owned(), book.title().to_owned());

            for record in found {
                let name = self.record_name(book, &record);
                let tag = records::tag(&record.path);
                let unit = self.unit(Who::Record(record.path), folder, tag, name);

                self.listed(unit, Pane::Records(book), book.base(), Source::Variants);
            }
        }

        let weights = Sheet::Weights;

        if let (Some(holder), Some(_)) = (root, vfs.find(weights.base())) {
            let unit = self.unit(Who::Global, holder, weights.base().to_owned(), weights.title().to_owned());

            self.rows(unit, Pane::Sheet(weights), weights.base(), Source::Variants);
        }

        if let Some(holder) = root {
            self.presets(holder);
        }
    }

    fn presets(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let Some(settings) = vfs.load(Book::MapSettings.base()).and_then(|bytes| RoguelikeMapSettings::parse(bytes.as_ref()).ok()) else {
            return;
        };

        let mut events: Vec<(&i32, &RoguelikeEvent)> = settings.events.iter().collect();

        events.sort_unstable_by_key(|(id, _)| **id);

        let mut users: Vec<(String, Vec<i32>)> = Vec::new();

        for (id, event) in events {
            for entry in &event.data {
                let file = entry.preset_file_name.trim();

                if file.is_empty() {
                    continue;
                }

                match users.iter_mut().find(|(held, _)| held == file) {
                    Some((_, ids)) if ids.contains(id) => {}
                    Some((_, ids)) => ids.push(*id),
                    None => users.push((file.to_owned(), vec![*id])),
                }
            }
        }

        if users.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), PRESET_FOLDER_RAW.to_owned(), preset::FOLDER.to_owned());

        for (file, ids) in users {
            let listed: Vec<String> = ids.iter().map(i32::to_string).collect();
            let name = format!("{PRESET_EVENT} {}", listed.join(LIST_JOINT));
            let unit = self.unit(Who::Global, folder, file.clone(), name);

            self.listed(unit, Pane::Preset, &file, Source::Single);
        }
    }

    fn record_name(&self, book: Book, record: &Record) -> String {
        let sources = self.sources;
        let number = |key: &str| record.field(key).and_then(|field| match &field.value {
            Value::Number(number) => number.parse::<i64>().ok(),
            _ => None,
        });
        let segment = |at: usize| record.path.get(at).map_or("", String::as_str);
        let last = record.path.last().map_or("", String::as_str);

        match book {
            Book::Buffs => record
                .field("RoguelikeBonusBuffName")
                .and_then(|field| match &field.value {
                    Value::Text(key) => sources.localizable.lookup(key),
                    _ => None,
                })
                .filter(|name| !name.trim().is_empty())
                .map_or_else(|| format!("{} {last}", book.noun()), str::to_owned),
            Book::Charas => {
                let cat = number("CharaID")
                    .and_then(|id| u32::try_from(id).ok())
                    .and_then(|id| sources.cats.iter().find(|cat| cat.id == id))
                    .map_or_else(|| format!("{} {last}", book.noun()), |cat| cat.display_name(0));

                match number("Level") {
                    Some(level) => format!("{cat} Lv{level}"),
                    None => cat,
                }
            }
            Book::StageLotteries => {
                let stage = match segment(2) {
                    "-1" => String::new(),
                    held => format!(" Stage {held}"),
                };
                let draw = last.parse::<u32>().map_or_else(|_| last.to_owned(), |draw| (draw + 1).to_string());

                format!("Map {}{stage} Draw {draw}", segment(1))
            }
            Book::Permanent => {
                let map = match segment(2) {
                    "-1" => "Every Map".to_owned(),
                    held => format!("Map {held}"),
                };

                format!("Event {} {map}", segment(1))
            }
            Book::MapSettings => record
                .field("PresetFileName")
                .and_then(|field| match &field.value {
                    Value::Text(name) => Some(format!("Event {} {name}", segment(1))),
                    _ => None,
                })
                .unwrap_or_else(|| format!("Event {} Entry {last}", segment(1))),
            Book::RuleBans => records::rule_label(last).map_or_else(|| format!("{} {last}", book.noun()), str::to_owned),
            Book::Orbs
            | Book::Medals
            | Book::Gauge
            | Book::MapConditions
            | Book::ClearRewards
            | Book::PointMaps
            | Book::PointUnlocks
            | Book::PointRewards
            | Book::EventDisplay
            | Book::MissionLimits
            | Book::GachaMissions
            | Book::GachaBonus
            | Book::LoginStamps
            | Book::Catseyes
            | Book::UnlockPopups
            | Book::RemoteConfig => {
                format!("{} {last}", book.noun())
            }
            Book::BuffSkills
            | Book::EffectAbilities
            | Book::LotteryGroups
            | Book::SpecialRules
            | Book::ScoreBonus
            | Book::PointRules => {
                format!("{} {last}", book.noun())
            }
        }
    }

    fn sheets(&mut self, parent: u32) {
        for kind in SHEETS.iter().copied() {
            let unit = self.unit(Who::Global, parent, kind.base().to_owned(), kind.title().to_owned());

            self.rows(unit, Pane::Sheet(kind), kind.base(), Source::Variants);
        }
    }

    fn book(&mut self, parent: u32, book: Book) {
        let vfs = &self.sources.vault.vfs;

        let Some(text) = vfs.find(book.base()).and_then(|path| vfs.read(&path).ok()).and_then(|bytes| String::from_utf8(bytes.to_vec()).ok()) else {
            return;
        };

        let entries = book.entries(&text);

        if entries.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), book.base().to_owned(), book.title().to_owned());
        let found = json::records(&text);

        let named = match book {
            Book::Orbs => orb_named(self.sources),
            _ => Vec::new(),
        };

        let medals = match book {
            Book::Medals => medal_names(self.sources),
            _ => Vec::new(),
        };

        for path in entries {
            let record = found.iter().find(|record| record.path == path);
            let index = path.last().map_or("", String::as_str);

            let name = match (book, record) {
                (Book::Catseyes, _) => records::catseye_name(index).unwrap_or_else(|| format!("{} {index}", book.noun())),
                (Book::Orbs, Some(record)) => orb_name(record, &named),
                (Book::Medals, _) => index
                    .parse::<usize>()
                    .ok()
                    .and_then(|at| medals.get(at))
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("{} {index}", book.noun())),
                (_, Some(record)) => self.record_name(book, record),
                (_, None) => format!("{} {index}", book.noun()),
            };

            let tag = records::tag(&path);
            let unit = self.unit(Who::Record(path), folder, tag, name);

            self.listed(unit, Pane::Records(book), book.base(), Source::Variants);
        }
    }

    fn whole(&mut self, parent: u32, book: Book) {
        if self.sources.vault.vfs.find(book.base()).is_none() {
            return;
        }

        let unit = self.unit(Who::Record(Vec::new()), parent, book.base().to_owned(), book.title().to_owned());

        self.listed(unit, Pane::Records(book), book.base(), Source::Variants);
    }

    fn missions(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let rewards = vfs.find(MISSION_TABLE).is_some();

        if !rewards && MISSION_BOOKS.iter().all(|book| vfs.find(book.base()).is_none()) {
            return;
        }

        let folder = self.folder(Some(parent), MISSIONS.to_owned(), MISSIONS.to_owned());

        for book in MISSION_BOOKS {
            self.book(folder, book);
        }

        if rewards {
            self.mission_rows(folder, MISSION_TABLE, MISSION_REWARDS, figures::Subject::Missions);
        }

        if vfs.find(CONDITION_TABLE).is_some() {
            self.mission_rows(folder, CONDITION_TABLE, MISSION_CONDITIONS, figures::Subject::Conditions);
        }

        self.keyed(folder, MISSION_UNLOCK_TABLE, MISSION_UNLOCKS, MISSION_UNLOCK_NAME, figures::Subject::MissionUnlocks);
        self.keyed(folder, MISSION_GROUP_TABLE, MISSION_GROUPS, MISSION_GROUP_NAME, figures::Subject::MissionGroups);
        self.keyed(folder, MISSION_SHORTCUT_TABLE, MISSION_SHORTCUTS, MISSION_SHORTCUT_NAME, figures::Subject::MissionShortcuts);
        self.keyed(folder, MISSION_DESIGNATION_TABLE, MISSION_DESIGNATIONS, MISSION_NAME, figures::Subject::MissionDesignations);
        self.keyed(folder, MONTHLY_MISSION_TABLE, MONTHLY_MISSIONS, MONTHLY_MISSION_NAME, figures::Subject::MonthlyMissions);

        let text = Sheet::MissionText;

        if !copies(&self.sources.vault.vfs, text.base()).is_empty() {
            let unit = self.unit(Who::Global, folder, text.base().to_owned(), text.title().to_owned());

            self.rows(unit, Pane::Sheet(text), text.base(), Source::Variants);
        }
    }

    fn mission_rows(&mut self, parent: u32, table: &str, heading: &str, subject: figures::Subject) {
        let vfs = &self.sources.vault.vfs;

        let Some(bytes) = vfs.load(table) else {
            return;
        };

        let ids: Vec<u32> = common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect();

        if ids.is_empty() {
            return;
        }

        let named = copies(vfs, MISSION_NAMES).into_iter().next().unwrap_or_else(|| MISSION_NAMES.to_owned());

        let names: FxHashMap<u32, String> = vfs.find(named.as_str()).and_then(|path| vfs.read(&path).ok()).map_or_else(FxHashMap::default, |bytes| {
            common::scrub(&bytes)
                .lines()
                .filter_map(|line| {
                    let (id, text) = line.split_once(['|', ','])?;
                    let shown = text.split(MISSION_BREAK).next()?.trim();

                    Some((id.trim().parse().ok()?, shown.to_owned()))
                })
                .collect()
        });

        let folder = self.folder(Some(parent), table.to_owned(), heading.to_owned());

        for id in ids {
            let name = names.get(&id).filter(|text| !text.is_empty()).map_or_else(|| format!("{MISSION_NAME} {id}"), |text| format!("{id}: {text}"));
            let unit = self.unit(Who::Part(id), folder, format!("{MISSION_NAME} {id:05}"), name);

            self.listed(unit, Pane::Figures(subject), table, Source::Variants);
        }
    }

    fn points(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let mut held: Vec<(Book, FxHashSet<u32>)> = Vec::new();

        for book in POINT_BOOKS {
            let ids: FxHashSet<u32> = vfs.load(book.base()).map_or_else(FxHashSet::default, |bytes| {
                let text = common::scrub(&bytes);

                json::objects(&text)
                    .into_iter()
                    .filter(|node| node.path.len() == 2 && node.path.first().is_some_and(|head| head == POINT_KEY))
                    .filter_map(|node| node.path.get(1)?.parse().ok())
                    .collect()
            });

            held.push((book, ids));
        }

        let mut ids: Vec<u32> = held.iter().flat_map(|(_, ids)| ids.iter().copied()).collect();

        ids.sort_unstable();
        ids.dedup();

        if ids.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), POINT_FOLDER_RAW.to_owned(), POINT_FOLDER.to_owned());

        for id in ids {
            let name = format!("{POINT_EVENT} {id}");
            let home = self.folder(Some(folder), id.to_string(), name.clone());
            let unit = self.unit(Who::Record(vec![POINT_KEY.to_owned(), id.to_string()]), home, id.to_string(), name);

            for (book, owned) in &held {
                match (owned.contains(&id), *book == Book::PointRewards) {
                    (true, true) => self.headed(unit, Pane::Records(*book), book.base(), REWARD_RESET),
                    (true, false) => self.rows(unit, Pane::Records(*book), book.base(), Source::Variants),
                    (false, _) => {}
                }
            }

            let rewards = Book::PointRewards;

            if !held.iter().any(|(book, owned)| *book == rewards && owned.contains(&id)) {
                continue;
            }

            let Some(text) = vfs.load(rewards.base()).map(|bytes| common::scrub(&bytes)) else {
                continue;
            };

            let listed = self.folder(Some(home), REWARD_FOLDER_RAW.to_owned(), REWARD_FOLDER.to_owned());
            let key = id.to_string();

            for record in json::records(&text) {
                let [head, event, group, index] = record.path.as_slice() else {
                    continue;
                };

                if head != POINT_KEY || *event != key || group != REWARD_FOLDER_RAW {
                    continue;
                }

                let points = record.field(REWARD_POINTS).map(|field| match &field.value {
                    Value::Number(number) => number.clone(),
                    _ => String::new(),
                });

                let name = match points {
                    Some(points) if !points.is_empty() => format!("{REWARD_NAME} {index}: {points} {REWARD_UNIT}"),
                    _ => format!("{REWARD_NAME} {index}"),
                };

                let tag = format!("{REWARD_NAME} {index:0>3}");
                let unit = self.unit(Who::Record(record.path.clone()), listed, tag, name);

                self.listed(unit, Pane::Records(rewards), rewards.base(), Source::Variants);
            }
        }
    }

    fn groups(&mut self, parent: u32) {
        let sources = self.sources;

        if sources.vault.vfs.find(group::TABLE).is_none() {
            return;
        }

        let table = sources.vault.vds.stages.charagroups(&sources.vault.vfs);
        let mut ids: Vec<u32> = table.keys().copied().collect();

        if ids.is_empty() {
            return;
        }

        ids.sort_unstable();

        let folder = self.folder(Some(parent), group::TABLE.to_owned(), group::FOLDER.to_owned());

        for id in ids {
            let name = table
                .get(&id)
                .and_then(|held| held.text_id.as_deref())
                .and_then(|key| sources.localizable.lookup(key))
                .map(str::trim)
                .filter(|words| !words.is_empty())
                .map_or_else(|| format!("{GROUP_NAME} {id}"), str::to_owned);

            let unit = self.unit(Who::Group(id), folder, format!("{GROUP_NAME} {id:03}"), name);

            self.listed(unit, Pane::Group, group::TABLE, Source::Variants);
        }
    }

    fn orb_traits(&mut self, parent: u32) {
        let sources = self.sources;

        let Some(bytes) = sources.vault.vfs.load(ORB_TRAIT_TABLE) else {
            return;
        };

        let lines: Vec<usize> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i32>().is_ok()))
            .map(|(at, _)| at)
            .collect();

        if lines.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), ORB_TRAIT_TABLE.to_owned(), ORB_TRAIT_FOLDER.to_owned());

        for (trait_id, line) in lines.into_iter().enumerate() {
            let name = orb::TRAITS.get(trait_id).map_or_else(|| format!("{ORB_TRAIT_LINE} {trait_id}"), |identity| sources.names.trait_label(*identity).to_owned());
            let unit = self.unit(Who::Line(line), folder, format!("{ORB_TRAIT_LINE} {trait_id:02}"), name);

            self.listed(unit, Pane::Figures(figures::Subject::OrbTraits), ORB_TRAIT_TABLE, Source::Variants);
        }
    }

    fn grades(&mut self, parent: u32) {
        if self.sources.vault.vfs.find(ORB_GRADE_TABLE).is_none() {
            return;
        }

        let folder = self.folder(Some(parent), ORB_GRADE_TABLE.to_owned(), ORB_GRADES.to_owned());

        for (at, grade) in orb::GRADES.iter().enumerate() {
            let line = at + 1;
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line}"), format!("{ORB_GRADE_LABEL} {grade}"));

            self.listed(unit, Pane::Figures(figures::Subject::OrbGrades), ORB_GRADE_TABLE, Source::Variants);
        }
    }

    fn text_folder(&mut self, parent: u32, heading: &str, kinds: &[Sheet]) {
        let present: Vec<Sheet> = kinds.iter().copied().filter(|kind| !copies(&self.sources.vault.vfs, kind.base()).is_empty()).collect();

        if present.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), heading.to_owned(), heading.to_owned());

        for kind in present {
            let unit = self.unit(Who::Global, folder, kind.title().to_owned(), kind.title().to_owned());

            self.rows(unit, Pane::Sheet(kind), kind.base(), Source::Languages);
        }
    }

    fn treasure_text(&mut self, parent: u32, slot: usize) {
        let held = |files: &[(usize, &'static str)]| files.iter().find(|(owner, _)| *owner == slot).map(|(_, file)| *file);
        let mut kinds: Vec<Sheet> = Vec::new();

        if let Some(file) = held(&TREASURE_NAMES) {
            kinds.push(Sheet::TreasureNames(file));
        }

        if let Some(file) = held(&TREASURE_SETS) {
            kinds.extend([Sheet::TreasureSets(file), Sheet::TreasureLeads(file), Sheet::TreasureEffects(file)]);
        }

        self.text_folder(parent, TREASURE_TEXT, &kinds);

        let (owner, retold) = TREASURE_RETOLD;

        if owner == slot {
            self.text_folder(parent, TREASURE_RETOLD_TEXT, &[Sheet::TreasureSets(retold), Sheet::TreasureLeads(retold), Sheet::TreasureEffects(retold)]);
        }
    }

    fn medal_text(&mut self, parent: u32) {
        let kinds = [Sheet::MedalNames, Sheet::MedalDescriptions];

        if copies(&self.sources.vault.vfs, Sheet::MedalNames.base()).is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), MEDAL_TEXT_RAW.to_owned(), MEDAL_TEXT.to_owned());

        for kind in kinds {
            let unit = self.unit(Who::Global, folder, kind.title().to_owned(), kind.title().to_owned());

            self.rows(unit, Pane::Sheet(kind), kind.base(), Source::Variants);
        }
    }

    fn combo_text(&mut self, parent: u32) {
        let kinds = [Sheet::ComboEffects, Sheet::ComboBands];

        if kinds.iter().all(|kind| copies(&self.sources.vault.vfs, kind.base()).is_empty()) {
            return;
        }

        let folder = self.folder(Some(parent), COMBO_TEXT.to_owned(), COMBO_TEXT.to_owned());

        for kind in kinds {
            let unit = self.unit(Who::Global, folder, kind.base().to_owned(), kind.title().to_owned());

            self.rows(unit, Pane::Sheet(kind), kind.base(), Source::Variants);
        }
    }

    fn filters(&mut self, parent: u32) {
        let sources = self.sources;

        let Some(bytes) = sources.vault.vfs.load(filter::TABLE) else {
            return;
        };

        let text = common::scrub(&bytes);
        let lines: Vec<&str> = text.lines().collect();

        if lines.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), filter::TABLE.to_owned(), filter::FOLDER.to_owned());

        for (line, raw) in lines.iter().enumerate() {
            let first = filter::listed(raw).first().and_then(|effect| usize::try_from(*effect).ok()).map(|effect| combo_effect(sources, effect));

            let name = match (line, first) {
                (0, _) => format!("{} 0: {FILTER_ALL}", filter::NOUN),
                (_, Some(effect)) => format!("{} {line}: {effect}", filter::NOUN),
                (_, None) => format!("{} {line}", filter::NOUN),
            };

            let unit = self.unit(Who::Line(line), folder, format!("{} {line:02}", filter::NOUN), name);

            self.listed(unit, Pane::Filter, filter::TABLE, Source::Variants);
        }
    }

    fn strengths(&mut self, parent: u32) {
        let sources = self.sources;
        let vfs = &sources.vault.vfs;

        let Some(rows) = vfs.find(COMBO_PARAM_TABLE).and_then(|path| vfs.read(&path).ok()).map(|bytes| common::scrub(&bytes).lines().count()) else {
            return;
        };

        if rows == 0 {
            return;
        }

        let folder = self.folder(Some(parent), COMBO_PARAM_TABLE.to_owned(), STRENGTHS.to_owned());

        for line in 0..rows {
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line}"), combo_effect(sources, line));

            self.listed(unit, Pane::Figures(figures::Subject::ComboParam), COMBO_PARAM_TABLE, Source::Variants);
        }
    }

    fn labyrinth(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let tables: BTreeSet<String> = vfs.glob(LABYRINTH_PREFIX).iter().filter(|name| name.ends_with(".csv")).map(|name| name.to_string()).collect();
        let mut root: Option<u32> = None;

        for table in tables {
            let Some(bytes) = vfs.load(&table) else {
                continue;
            };

            let lines: Vec<usize> = common::scrub(&bytes)
                .lines()
                .enumerate()
                .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                .map(|(at, _)| at)
                .collect();

            if lines.is_empty() {
                continue;
            }

            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), LABYRINTH_FOLDER.to_owned(), LABYRINTH_FOLDER.to_owned()));
            let folder = self.folder(Some(holder), table.clone(), table.trim_end_matches(".csv").to_owned());

            for (position, line) in lines.into_iter().enumerate() {
                let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:03}"), format!("{LABYRINTH_FLOOR_NAME} {}", position + 1));

                self.listed(unit, Pane::Figures(figures::Subject::LabyrinthFloors), &table, Source::Variants);
            }
        }

        if vfs.find(LABYRINTH_LOTTERY_TABLE).is_some() {
            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), LABYRINTH_FOLDER.to_owned(), LABYRINTH_FOLDER.to_owned()));

            self.keyed(holder, LABYRINTH_LOTTERY_TABLE, LABYRINTH_LOTTERY_FOLDER, LABYRINTH_RULE_NAME, figures::Subject::LabyrinthLottery);
        }
    }

    fn realms(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let tables: BTreeSet<String> = vfs.glob(REALM_PREFIX).iter().filter(|name| name.ends_with(".csv")).map(|name| name.to_string()).collect();
        let mut root: Option<u32> = None;

        for table in tables {
            let Some(bytes) = vfs.load(&table) else {
                continue;
            };

            let lines: Vec<usize> = common::scrub(&bytes)
                .lines()
                .enumerate()
                .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                .map(|(at, _)| at)
                .collect();

            let Some((&altar, draws)) = lines.split_first() else {
                continue;
            };

            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), REALM_FOLDER.to_owned(), REALM_FOLDER.to_owned()));
            let folder = self.folder(Some(holder), table.clone(), table.trim_end_matches(".csv").to_owned());
            let unit = self.unit(Who::Line(altar), folder, format!("{LINE_TAG} {altar:02}"), REALM_ALTAR_NAME.to_owned());

            self.listed(unit, Pane::Figures(figures::Subject::RealmAltar), &table, Source::Variants);

            for (position, line) in draws.iter().enumerate() {
                let unit = self.unit(Who::Line(*line), folder, format!("{LINE_TAG} {line:02}"), format!("{REALM_DRAW_NAME} {}", position + 1));

                self.listed(unit, Pane::Figures(figures::Subject::RealmDraws), &table, Source::Variants);
            }
        }
    }

    fn quest(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let mut root: Option<u32> = None;

        for (prefix, heading, noun, subject) in [
            (QUEST_STAGE_PREFIX, QUEST_STAGE_FOLDER, QUEST_STAGE_NAME, figures::Subject::QuestStages),
            (SLOT_LOTTERY_PREFIX, SLOT_LOTTERY_FOLDER, SLOT_LOTTERY_NAME, figures::Subject::SlotLottery),
        ] {
            let tables: BTreeSet<String> = vfs.glob(prefix).iter().filter(|name| name.ends_with(".csv")).map(|name| name.to_string()).collect();

            for table in tables {
                let Some(bytes) = vfs.load(&table) else {
                    continue;
                };

                let lines: Vec<usize> = common::scrub(&bytes)
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                    .map(|(at, _)| at)
                    .collect();

                if lines.is_empty() {
                    continue;
                }

                let holder = *root.get_or_insert_with(|| self.folder(Some(parent), QUEST_FOLDER.to_owned(), QUEST_FOLDER.to_owned()));
                let folder = self.folder(Some(holder), table.clone(), format!("{heading} ({})", table.trim_start_matches(prefix).trim_end_matches(".csv")));
                let last = lines.len().saturating_sub(1);

                for (position, line) in lines.into_iter().enumerate() {
                    let named = match (subject, position == last) {
                        (figures::Subject::SlotLottery, true) => format!("{position}+ {noun}"),
                        (figures::Subject::SlotLottery, false) => format!("{position} {noun}"),
                        _ => format!("{noun} {}", position + 1),
                    };

                    let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), named);

                    self.listed(unit, Pane::Figures(subject), &table, Source::Variants);
                }
            }
        }
    }

    fn map_layouts(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let tables: BTreeSet<String> = vfs
            .glob(maplayout::PREFIX)
            .iter()
            .filter(|name| name.strip_prefix(maplayout::PREFIX).and_then(|rest| rest.strip_suffix(".csv")).is_some_and(|digits| digits.chars().all(|glyph| glyph.is_ascii_digit())))
            .map(|name| name.to_string())
            .collect();

        let stacked: Vec<(&str, &str)> = dungeon::TABLES.into_iter().filter(|(table, _)| vfs.find(*table).is_some()).collect();

        if tables.is_empty() && stacked.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), maplayout::PREFIX.to_owned(), maplayout::FOLDER.to_owned());

        for (table, named) in stacked {
            let unit = self.unit(Who::Global, folder, table.to_owned(), named.to_owned());

            self.listed(unit, Pane::Dungeon, table, Source::Variants);
        }

        for table in tables {
            let number = table.trim_start_matches(maplayout::PREFIX).trim_end_matches(".csv").to_owned();
            let unit = self.unit(Who::Global, folder, table.clone(), format!("{} {number}", maplayout::TITLE));

            self.listed(unit, Pane::MapLayout, &table, Source::Variants);
        }
    }

    fn castles(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let mut root: Option<u32> = None;

        for (table, heading, prefix) in CASTLE_SETS {
            let Some(bytes) = vfs.load(table) else {
                continue;
            };

            let rows = common::scrub(&bytes)
                .lines()
                .map(|line| line.split(',').next().and_then(|cell| cell.trim().parse::<i32>().ok()))
                .take_while(|first| *first != Some(CASTLE_END))
                .count();

            if rows == 0 {
                continue;
            }

            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), CASTLE_FOLDER.to_owned(), CASTLE_FOLDER.to_owned()));
            let folder = self.folder(Some(holder), table.to_owned(), heading.to_owned());

            for line in 0..rows {
                let tag = format!("{LINE_TAG} {line:03}");
                let named = format!("{CASTLE_NAME} {line}");
                let home = self.folder(Some(folder), tag.clone(), named.clone());
                let unit = self.unit(Who::Line(line), home, tag, named);

                self.rows(unit, Pane::Figures(figures::Subject::Castles), table, Source::Variants);
                self.rows(unit, Pane::Artwork(Art::Castle), &format!("{prefix}{line:03}.png"), Source::Languages);
            }
        }

        let mut strays = self.strays(CASTLE_CUT_PREFIX, castle_stem);

        for (_, _, prefix) in CASTLE_SETS {
            strays.extend(self.strays(prefix, castle_stem));
        }

        if strays.is_empty() {
            return;
        }

        let holder = root.unwrap_or_else(|| self.folder(Some(parent), CASTLE_FOLDER.to_owned(), CASTLE_FOLDER.to_owned()));

        self.shelve(holder, CASTLE_STRAYS, strays);
    }

    fn gacha(&mut self, parent: u32) {
        let sources = self.sources;
        let vfs = &sources.vault.vfs;
        let mut root: Option<u32> = None;

        for (capsule, heading) in roster::CAPSULES {
            let tiers: Vec<(roster::Tier, String)> =
                roster::TIERS.into_iter().map(|tier| (tier, tier.file(capsule))).filter(|(_, file)| vfs.find(file).is_some()).collect();

            let Some(lines) = tiers.first().and_then(|(_, file)| vfs.load(file)).map(|bytes| common::scrub(&bytes).lines().count()) else {
                continue;
            };

            let options = roster::options_file(capsule);

            let keyed: FxHashSet<u32> = vfs.load(&options).map_or_else(FxHashSet::default, |bytes| {
                common::scrub(&bytes).lines().filter_map(|line| line.split('\t').next()?.trim().parse().ok()).collect()
            });

            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), roster::FOLDER.to_owned(), roster::FOLDER.to_owned()));

            self.homes.insert(roster::FOLDER, holder);
            let folder = self.folder(Some(holder), format!("{}{capsule}", roster::PREFIX), heading.to_owned());

            for line in 0..lines {
                let tag = format!("{} {line:04}", roster::SET_NAME);

                let named = format!("{} {line}", roster::SET_NAME);

                let set = self.folder(Some(folder), tag.clone(), named.clone());
                let unit = self.unit(Who::Line(line), set, tag.clone(), named.clone());

                for (tier, file) in &tiers {
                    self.rows(unit, Pane::Roster(*tier), file, Source::Variants);
                }

                let Some(id) = u32::try_from(line).ok().filter(|id| keyed.contains(id)) else {
                    continue;
                };

                let unit = self.unit(Who::Part(id), set, tag, named);

                self.rows(unit, Pane::Figures(figures::Subject::GachaOptions), &options, Source::Variants);
            }
        }

        let Some(bytes) = vfs.load(roster::CHANCE_TABLE) else {
            return;
        };

        let rules: Vec<(usize, Option<i64>, Option<i64>)> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .filter_map(|(line, text)| {
                let mut cells = text.split('\t').map(|cell| cell.trim().parse::<i64>().ok());
                let capsule = cells.next()?;

                capsule.map(|_| (line, capsule, cells.next().flatten()))
            })
            .collect();

        if rules.is_empty() {
            return;
        }

        let holder = *root.get_or_insert_with(|| self.folder(Some(parent), roster::FOLDER.to_owned(), roster::FOLDER.to_owned()));
        let folder = self.folder(Some(holder), roster::CHANCE_TABLE.to_owned(), roster::CHANCE_FOLDER.to_owned());

        for (position, (line, capsule, series)) in rules.into_iter().enumerate() {
            let number = position + 1;
            let named = roster::chance_name(capsule, series).unwrap_or_else(|| format!("{} {number}", roster::CHANCE_NAME));
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line}"), named);

            self.listed(unit, Pane::Figures(figures::Subject::ChanceAnimations), roster::CHANCE_TABLE, Source::Variants);
        }
    }

    fn treasures(&mut self, parent: u32) {
        let sources = self.sources;
        let catalog = sources.vault.vds.treasures.catalog(&sources.vault.vfs);
        let mut root: Option<u32> = None;

        for (slot, arc) in treasure::ARCS {
            let Some(file) = data_file(slot) else {
                continue;
            };

            if sources.vault.vfs.find(file.as_str()).is_none() {
                continue;
            }

            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), treasure::FOLDER.to_owned(), treasure::FOLDER.to_owned()));
            let folder = self.folder(Some(holder), file.clone(), arc.to_owned());


            for group in 0..GROUPS {
                let name = catalog
                    .set(slot, group)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map_or_else(|| format!("{SET_NAME} {group}"), str::to_owned);

                let unit = self.unit(Who::Line(group), folder, format!("{SET_NAME} {group:02}"), name);

                self.listed(unit, Pane::Treasure, &file, Source::Variants);
            }

            self.treasure_text(folder, slot);
        }

        if let Some(holder) = root {
            let labels = Sheet::TreasureLabels;

            if !copies(&sources.vault.vfs, labels.base()).is_empty() {
                let unit = self.unit(Who::Global, holder, labels.base().to_owned(), labels.title().to_owned());

                self.rows(unit, Pane::Sheet(labels), labels.base(), Source::Languages);
            }
        }

        let gauge = Book::Gauge;

        if let (Some(holder), Some(_)) = (root, sources.vault.vfs.find(gauge.base())) {
            let unit = self.unit(Who::Record(Vec::new()), holder, gauge.base().to_owned(), gauge.title().to_owned());

            self.listed(unit, Pane::Records(gauge), gauge.base(), Source::Variants);
        }
    }

    fn keyed(&mut self, parent: u32, table: &str, heading: &str, noun: &str, subject: figures::Subject) {
        let Some(bytes) = self.sources.vault.vfs.load(table) else {
            return;
        };

        let ids: Vec<u32> = common::scrub(&bytes).lines().filter_map(|line| line.split([',', '\t']).next()?.trim().parse().ok()).collect();

        if ids.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), table.to_owned(), heading.to_owned());

        for id in ids {
            let unit = self.unit(Who::Part(id), folder, format!("{noun} {id:03}"), format!("{noun} {id}"));

            self.listed(unit, Pane::Figures(subject), table, Source::Variants);
        }
    }

    fn numbered(&mut self, parent: u32, table: &str, heading: &str, noun: &str, subject: figures::Subject, skip: usize) {
        let Some(bytes) = self.sources.vault.vfs.load(table) else {
            return;
        };

        let lines: Vec<usize> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .skip(skip)
            .filter(|(_, line)| line.split([',', '\t']).next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
            .map(|(at, _)| at)
            .collect();

        if lines.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), table.to_owned(), heading.to_owned());

        for (position, line) in lines.into_iter().enumerate() {
            let number = position + 1;
            let unit = self.unit(Who::Line(line), folder, format!("{noun} {number:02}"), format!("{noun} {number}"));

            self.listed(unit, Pane::Figures(subject), table, Source::Variants);
        }
    }

    fn gamatoto(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let limit = vfs.find(GAMATOTO_LIMIT_TABLE).is_some();

        let keyed = |table: &str| -> Vec<u32> {
            vfs.load(table).map_or_else(Vec::new, |bytes| {
                common::scrub(&bytes)
                    .lines()
                    .filter_map(|line| line.split([',', '\t']).next()?.trim().parse().ok())
                    .collect()
            })
        };

        let unlocks = keyed(GAMATOTO_UNLOCK_TABLE);
        let drops = keyed(GAMATOTO_DROP_TABLE);

        let worded = GAMATOTO_SHEETS.iter().any(|kind| !copies(vfs, kind.base()).is_empty())
            || GAMATOTO_CAPTIONS.iter().any(|layout| !copies(vfs, layout.base()).is_empty());

        let numbered = |table: &str| -> Vec<usize> {
            vfs.load(table).map_or_else(Vec::new, |bytes| {
                common::scrub(&bytes)
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                    .map(|(at, _)| at)
                    .collect()
            })
        };
        let areas = EXPEDITION_AREA_TABLES.map(|(table, heading)| (table, heading, numbered(table)));
        let levels = numbered(GAMATOTO_LEVEL_TABLE);
        let zoned = numbered(ZONED_IN_TABLE);
        let tabled = !levels.is_empty() || !zoned.is_empty() || areas.iter().any(|(_, _, lines)| !lines.is_empty());

        if !limit && unlocks.is_empty() && drops.is_empty() && !worded && !tabled {
            return;
        }

        let folder = self.folder(Some(parent), GAMATOTO_FOLDER.to_owned(), GAMATOTO_FOLDER.to_owned());

        self.homes.insert(GAMATOTO_FOLDER, folder);

        if limit {
            let unit = self.unit(Who::Line(0), folder, GAMATOTO_LIMIT_TABLE.to_owned(), GAMATOTO_LIMIT_NAME.to_owned());

            self.listed(unit, Pane::Figures(figures::Subject::GamatotoLimit), GAMATOTO_LIMIT_TABLE, Source::Variants);
        }

        for (table, heading, noun, subject, ids) in [
            (GAMATOTO_UNLOCK_TABLE, GAMATOTO_UNLOCKS, GAMATOTO_UNLOCK_NAME, figures::Subject::GamatotoUnlock, unlocks),
            (GAMATOTO_DROP_TABLE, GAMATOTO_DROPS, GAMATOTO_DROP_NAME, figures::Subject::GamatotoDrop, drops),
        ] {
            if ids.is_empty() {
                continue;
            }

            let held = self.folder(Some(folder), table.to_owned(), heading.to_owned());

            for id in ids {
                let unit = self.unit(Who::Part(id), held, format!("{noun} {id:02}"), format!("{noun} {id}"));

                self.listed(unit, Pane::Figures(subject), table, Source::Variants);
            }
        }

        for (table, heading, lines) in areas {
            if lines.is_empty() {
                continue;
            }

            let held = self.folder(Some(folder), table.to_owned(), heading.to_owned());

            for (position, line) in lines.into_iter().enumerate() {
                let unit = self.unit(Who::Line(line), held, format!("{LINE_TAG} {line:02}"), format!("{EXPEDITION_AREA_NAME} {}", position + 1));

                self.listed(unit, Pane::Figures(figures::Subject::ExpeditionAreas), table, Source::Variants);
            }
        }

        if !levels.is_empty() {
            let held = self.folder(Some(folder), GAMATOTO_LEVEL_TABLE.to_owned(), GAMATOTO_LEVEL_FOLDER.to_owned());

            for (position, line) in levels.into_iter().enumerate() {
                let unit = self.unit(Who::Line(line), held, format!("{LINE_TAG} {line:03}"), format!("{GAMATOTO_LEVEL_NAME} {}", position + 1));

                self.listed(unit, Pane::Figures(figures::Subject::GamatotoLevels), GAMATOTO_LEVEL_TABLE, Source::Variants);
            }
        }

        for (line, named, subject) in zoned.into_iter().zip([(ZONED_IN_NAME, figures::Subject::ZonedIn), (ZONED_IN_STEPS_NAME, figures::Subject::ZonedInSteps)]).map(|(line, (named, subject))| (line, named, subject)) {
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), named.to_owned());

            self.listed(unit, Pane::Figures(subject), ZONED_IN_TABLE, Source::Variants);
        }

        self.text_folder(folder, GAMATOTO_TEXT, &GAMATOTO_SHEETS);

        for layout in GAMATOTO_CAPTIONS {
            self.captions(folder, layout);
        }

        self.captions(folder, captions::Layout::GamatotoCollabs);
    }

    fn rank_text(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let messages = Sheet::RankMessages;
        let rewards = captions::Layout::RankRewards;

        if copies(vfs, messages.base()).is_empty() && copies(vfs, rewards.base()).is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), RANK_TEXT.to_owned(), RANK_TEXT.to_owned());

        if !copies(vfs, messages.base()).is_empty() {
            let unit = self.unit(Who::Global, folder, messages.title().to_owned(), messages.title().to_owned());

            self.languages(unit, Pane::Sheet(messages), messages.base());
        }

        self.captions(folder, rewards);
    }

    fn interface(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let present = INTERFACE_SHEETS.iter().any(|kind| !copies(vfs, kind.base()).is_empty())
            || captions::INTERFACE.iter().any(|layout| !copies(vfs, layout.base()).is_empty())
            || vfs.find(TIP_RULE_TABLE).is_some();

        if !present {
            return;
        }

        let folder = self.folder(Some(parent), INTERFACE_TEXT.to_owned(), INTERFACE_TEXT.to_owned());

        for kind in INTERFACE_SHEETS {
            if copies(vfs, kind.base()).is_empty() {
                continue;
            }

            let unit = self.unit(Who::Global, folder, kind.base().to_owned(), kind.title().to_owned());

            self.languages(unit, Pane::Sheet(kind), kind.base());
        }

        for layout in captions::INTERFACE {
            self.captions(folder, layout);
        }

        self.keyed(folder, TIP_RULE_TABLE, TIP_RULE_FOLDER, TIP_RULE_NAME, figures::Subject::TipRules);
    }

    fn story(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let kinds: Vec<Sheet> = STORIES.iter().map(|(file, _)| Sheet::Story(file)).filter(|kind| !copies(vfs, kind.base()).is_empty()).collect();
        let unlocks = captions::Layout::ChapterUnlocks;

        if kinds.is_empty() && copies(vfs, unlocks.base()).is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), STORY.to_owned(), STORY.to_owned());

        for kind in kinds {
            let unit = self.unit(Who::Global, folder, kind.base().to_owned(), kind.title().to_owned());

            self.languages(unit, Pane::Sheet(kind), kind.base());
        }

        self.captions(folder, unlocks);
    }

    fn login_messages(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let ids: BTreeSet<u32> = vfs
            .glob(LOGIN_PREFIX)
            .iter()
            .filter_map(|name| name.strip_prefix(LOGIN_PREFIX)?.split(['_', '.']).next()?.parse().ok())
            .collect();

        if ids.is_empty() {
            return;
        }

        let kind = Sheet::LoginMessages;
        let folder = self.folder(Some(parent), LOGIN_PREFIX.to_owned(), kind.title().to_owned());

        for id in ids {
            let base = format!("{LOGIN_PREFIX}{id}.{LOGIN_EXTENSION}");
            let unit = self.unit(Who::Global, folder, base.clone(), format!("{LOGIN_EVENT} {id}"));

            self.languages(unit, Pane::Sheet(kind), &base);
        }
    }

    fn sounds(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(SOUND_TABLE) else {
            return;
        };

        let lines: Vec<usize> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .filter(|(_, line)| line.split('\t').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
            .map(|(at, _)| at)
            .collect();

        if lines.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), SOUND_TABLE.to_owned(), SOUND_FOLDER.to_owned());

        for (position, line) in lines.into_iter().enumerate() {
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:03}"), format!("{SOUND_NAME} {position:03}"));

            self.listed(unit, Pane::Figures(figures::Subject::SoundSettings), SOUND_TABLE, Source::Variants);
        }
    }

    fn prizes(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        for (prefix, heading, noun) in PRIZE_SETS {
            let tables: BTreeSet<String> = vfs.glob(prefix).iter().filter(|name| name.ends_with(".csv")).map(|name| name.to_string()).collect();
            let mut root: Option<u32> = None;

            for table in tables {
                let number = table.trim_start_matches(prefix).trim_end_matches(".csv").to_owned();
                let rows: Vec<(usize, String)> = vfs.load(&table).map_or_else(Vec::new, |bytes| {
                    common::scrub(&bytes)
                        .lines()
                        .enumerate()
                        .filter_map(|(at, line)| line.split(',').next()?.trim().parse::<i64>().ok().map(|attempt| (at, attempt.to_string())))
                        .collect()
                });

                if rows.is_empty() || !number.chars().all(|glyph| glyph.is_ascii_digit()) {
                    continue;
                }

                let holder = *root.get_or_insert_with(|| self.folder(Some(parent), prefix.to_owned(), heading.to_owned()));

                self.homes.insert(heading, holder);

                let folder = self.folder(Some(holder), table.clone(), format!("{noun} {number}"));

                for (position, (line, attempt)) in rows.into_iter().enumerate() {
                    let named = format!("{PRIZE_NAME} {}: {PRIZE_ATTEMPT} {attempt}", position + 1);
                    let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), named);

                    self.listed(unit, Pane::Figures(figures::Subject::PrizePatterns), &table, Source::Variants);
                }
            }
        }
    }

    fn reels(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let tables: BTreeSet<String> = vfs.glob(REEL_PREFIX).iter().filter(|name| name.ends_with(".csv")).map(|name| name.to_string()).collect();

        for table in tables {
            let number = table.trim_start_matches(REEL_PREFIX).trim_end_matches(".csv").to_owned();
            let lines: Vec<usize> = vfs.load(&table).map_or_else(Vec::new, |bytes| {
                common::scrub(&bytes)
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                    .map(|(at, _)| at)
                    .collect()
            });

            if lines.is_empty() || !numbered_stem(&number) {
                continue;
            }

            let holder = self.home(parent, SLOT_MACHINES);
            let folder = self.folder(Some(holder), table.clone(), format!("{REEL_FOLDER} {number}"));

            for (position, line) in lines.into_iter().enumerate() {
                let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), format!("{REEL_NAME} {}", position + 1));

                self.listed(unit, Pane::Figures(figures::Subject::ReelSymbols), &table, Source::Variants);
            }
        }
    }

    fn images(&mut self, parent: u32) {
        let mut pictures = std::mem::take(&mut self.pictures);
        let mut cuts = std::mem::take(&mut self.cuts);

        pictures.retain(|name| !rigged(&self.rigs, name));
        cuts.retain(|name| !rigged(&self.rigs, name));

        let mut root: Option<u32> = None;

        for (mut names, heading, pane) in [(pictures, PICTURES, Pane::Artwork(Art::Image)), (cuts, CUT_LISTS, Pane::Cuts)] {
            names.retain(|name| !self.files_at.contains_key(name.as_str()));
            names.sort_unstable();

            let mut shelf: Option<u32> = None;
            let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();

            for name in names {
                match IMAGE_PARTNERS.iter().find(|(prefix, _)| name.starts_with(prefix)) {
                    Some((_, home)) => {
                        let folder = self.partner(parent, home);
                        let unit = self.unit(Who::Global, folder, name.clone(), name.clone());

                        self.listed(unit, pane, &name, Source::Single);
                    }
                    None => groups.entry(family(&name)).or_default().push(name),
                }
            }

            for (stem, members) in groups {
                let images = *root.get_or_insert_with(|| self.folder(Some(parent), IMAGES.to_owned(), IMAGES.to_owned()));
                let held = *shelf.get_or_insert_with(|| self.folder(Some(images), heading.to_owned(), heading.to_owned()));
                let folder = match members.len() > 1 {
                    true => self.folder(Some(held), stem.clone(), stem),
                    false => held,
                };

                for name in members {
                    let unit = self.unit(Who::Global, folder, name.clone(), name.clone());

                    self.listed(unit, pane, &name, Source::Single);
                }
            }
        }
    }

    fn files(&mut self, parent: u32) {
        let mut shelves: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();

        for name in self.sources.vault.vfs.glob("").iter() {
            if self.files_at.contains_key(name.as_ref()) {
                continue;
            }

            let kind = name.rsplit_once('.').map_or_else(|| PLAIN_FILES.to_owned(), |(_, extension)| extension.to_ascii_uppercase());

            shelves.entry(kind).or_default().entry(family(name)).or_default().push(name.to_string());
        }

        if shelves.is_empty() {
            return;
        }

        let root = self.folder(Some(parent), FILES.to_owned(), FILES.to_owned());

        for (kind, groups) in shelves {
            let shelf = self.folder(Some(root), kind.clone(), kind);

            for (stem, members) in groups {
                let folder = match members.len() > 1 {
                    true => self.folder(Some(shelf), stem.clone(), stem),
                    false => shelf,
                };

                for name in members {
                    let unit = self.unit(Who::Global, folder, name.clone(), name.clone());

                    self.listed(unit, Pane::File, &name, Source::Single);
                }
            }
        }
    }

    fn sound_files(&mut self, parent: u32) {
        let mut names = std::mem::take(&mut self.sounds);

        names.retain(|name| !self.files_at.contains_key(name.as_str()));
        names.sort_unstable();

        let mut root: Option<u32> = None;
        let mut shelves: BTreeMap<String, u32> = BTreeMap::new();

        for name in names {
            let Some(kind) = name.rsplit_once('.').map(|(_, extension)| extension.to_ascii_uppercase()) else {
                continue;
            };

            let held = *root.get_or_insert_with(|| self.folder(Some(parent), SOUND_FILES.to_owned(), SOUND_FILES.to_owned()));
            let shelf = match shelves.get(&kind) {
                Some(shelf) => *shelf,
                None => {
                    let shelf = self.folder(Some(held), kind.clone(), kind.clone());

                    shelves.insert(kind, shelf);
                    shelf
                }
            };
            let unit = self.unit(Who::Global, shelf, name.clone(), name.clone());

            self.listed(unit, Pane::Audio, &name, Source::Single);
        }
    }

    fn home(&mut self, parent: u32, home: &'static str) -> u32 {
        match (home == CAT_BASE, self.homes.get(home).copied()) {
            (true, _) => {
                let held = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

                self.cat_base = Some(held);
                held
            }
            (false, Some(held)) => held,
            (false, None) => {
                let held = self.folder(Some(parent), home.to_owned(), home.to_owned());

                self.homes.insert(home, held);
                held
            }
        }
    }

    fn map_markers(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let tables: BTreeSet<String> = vfs
            .glob(MAP_MARKER_PREFIX)
            .iter()
            .filter(|name| name.strip_prefix(MAP_MARKER_PREFIX).and_then(|rest| rest.strip_suffix(".csv")).is_some_and(|digits| !digits.is_empty() && digits.chars().all(|glyph| glyph.is_ascii_digit())))
            .map(|name| name.to_string())
            .collect();
        let mut root: Option<u32> = None;

        for table in tables {
            let lines: Vec<usize> = vfs.load(&table).map_or_else(Vec::new, |bytes| {
                common::scrub(&bytes)
                    .lines()
                    .enumerate()
                    .take(MAP_MARKER_ROWS)
                    .filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                    .map(|(at, _)| at)
                    .collect()
            });

            if lines.is_empty() {
                continue;
            }

            let number = table.trim_start_matches(MAP_MARKER_PREFIX).trim_end_matches(".csv").to_owned();
            let holder = *root.get_or_insert_with(|| self.folder(Some(parent), MAP_MARKER_PREFIX.to_owned(), MAP_MARKER_FOLDER.to_owned()));
            let folder = self.folder(Some(holder), table.clone(), format!("{MAP_MARKER_SET} {number}"));

            for line in lines {
                let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), format!("{MAP_MARKER_NAME} {line}"));

                self.listed(unit, Pane::Figures(figures::Subject::MapMarkers), &table, Source::Variants);
            }
        }
    }

    fn enigma(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let sets = [
            (ENIGMA_LOTTERY_TABLE, ENIGMA_LOTTERY_FOLDER, ENIGMA_LOTTERY_NAME, figures::Subject::EnigmaLotteries, 0),
            (ENIGMA_RATE_TABLE, ENIGMA_RATE_FOLDER, ENIGMA_LEVEL_NAME, figures::Subject::EnigmaRates, 1),
            (ENIGMA_DROP_TABLE, ENIGMA_DROP_FOLDER, ENIGMA_LEVEL_NAME, figures::Subject::EnigmaDrops, 1),
            (ENIGMA_GROUP_TABLE, ENIGMA_GROUP_FOLDER, ENIGMA_GROUP_NAME, figures::Subject::EnigmaGroups, 1),
            (ENIGMA_MEDAL_TABLE, ENIGMA_MEDAL_FOLDER, ENIGMA_MEDAL_NAME, figures::Subject::DiscoveryMedals, 1),
            (ENIGMA_TIME_TABLE, ENIGMA_TIME_FOLDER, ENIGMA_TIME_NAME, figures::Subject::EnigmaTimes, 0),
            (ENIGMA_RARITY_TABLE, ENIGMA_RARITY_FOLDER, ENIGMA_RARITY_NAME, figures::Subject::EnigmaRarities, 0),
        ];

        if !sets.iter().any(|(table, ..)| vfs.find(*table).is_some()) {
            return;
        }

        let folder = self.folder(Some(parent), ENIGMA_FOLDER.to_owned(), ENIGMA_FOLDER.to_owned());

        for (table, heading, noun, subject, first) in sets {
            let Some(bytes) = self.sources.vault.vfs.load(table) else {
                continue;
            };

            let lines: Vec<usize> = common::scrub(&bytes)
                .lines()
                .enumerate()
                .take_while(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok()))
                .map(|(at, _)| at)
                .collect();

            if lines.is_empty() {
                continue;
            }

            let held = self.folder(Some(folder), table.to_owned(), heading.to_owned());

            for (position, line) in lines.into_iter().enumerate() {
                let number = position + first;
                let unit = self.unit(Who::Line(line), held, format!("{LINE_TAG} {line:02}"), format!("{noun} {number}"));

                self.listed(unit, Pane::Figures(subject), table, Source::Variants);
            }
        }
    }

    fn fanfares(&mut self, parent: u32) {
        if self.sources.vault.vfs.find(FANFARE_TABLE).is_none() {
            return;
        }

        let folder = self.home(parent, roster::FOLDER);

        self.numbered(folder, FANFARE_TABLE, FANFARE_FOLDER, FANFARE_NAME, figures::Subject::FanfareTiers, 0);
    }

    fn shrine_tables(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let present = [SHRINE_OFFERING_TABLE, SHRINE_EXTRA_TABLE, FORTUNE_TABLE].map(|table| vfs.find(table).is_some());

        if !present.contains(&true) {
            return;
        }

        let folder = self.home(parent, SHRINE);

        if present[0] {
            self.keyed(folder, SHRINE_OFFERING_TABLE, SHRINE_OFFERING_FOLDER, SHRINE_OFFERING_NAME, figures::Subject::ShrineOfferings);
        }

        if present[1] {
            self.numbered(folder, SHRINE_EXTRA_TABLE, SHRINE_EXTRA_FOLDER, SHRINE_EXTRA_NAME, figures::Subject::ShrineExtras, 0);
        }

        if present[2] {
            self.keyed(folder, FORTUNE_TABLE, FORTUNE_FOLDER, FORTUNE_NAME, figures::Subject::FortuneOdds);
        }
    }

    fn partner(&mut self, parent: u32, home: &'static str) -> u32 {
        let key = (home, IMAGES);

        if let Some(folder) = self.galleries.get(&key) {
            return *folder;
        }

        let holder = self.home(parent, home);
        let folder = self.folder(Some(holder), IMAGES.to_owned(), IMAGES.to_owned());

        self.galleries.insert(key, folder);

        folder
    }

    fn login_events(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(logins::TABLE) else {
            return;
        };

        let events: Vec<(usize, String)> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .filter_map(|(at, line)| line.split(',').next()?.trim().parse::<i64>().ok().map(|id| (at, id.to_string())))
            .collect();

        if events.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), logins::TABLE.to_owned(), logins::FOLDER.to_owned());

        self.homes.insert(logins::FOLDER, folder);

        for (line, id) in events {
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:03}"), format!("{} {id}", logins::TITLE));

            self.listed(unit, Pane::Logins, logins::TABLE, Source::Variants);
        }
    }

    fn unused(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let held: Vec<&str> = unused::FILES.into_iter().map(|(name, _)| name).filter(|name| vfs.find(*name).is_some()).collect();
        let families: Vec<(&str, Vec<Box<str>>)> = unused::FAMILIES
            .into_iter()
            .map(|(prefix, suffix, label)| {
                let names = vfs.glob(prefix).into_iter().filter(|name| name.to_ascii_lowercase().ends_with(suffix)).collect();

                (label, names)
            })
            .filter(|(_, names): &(&str, Vec<Box<str>>)| !names.is_empty())
            .collect();

        if held.is_empty() && families.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), unused::FOLDER.to_owned(), unused::FOLDER.to_owned());

        for name in held {
            let unit = self.unit(Who::Global, folder, name.to_owned(), name.to_owned());

            self.listed(unit, Pane::Unused, name, Source::Single);
        }

        for (label, names) in families {
            let family = self.folder(Some(folder), label.to_owned(), label.to_owned());

            for name in names {
                let unit = self.unit(Who::Global, family, name.to_string(), name.to_string());

                self.listed(unit, Pane::Unused, &name, Source::Single);
            }
        }
    }

    fn chests(&mut self, parent: u32) {
        let sources = self.sources;

        let Some(bytes) = sources.vault.vfs.load(CHEST_TABLE) else {
            return;
        };

        let ids: Vec<u32> = common::scrub(&bytes).lines().filter_map(|line| line.split('\t').next()?.trim().parse().ok()).collect();

        if ids.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), CHEST_TABLE.to_owned(), CHEST_FOLDER.to_owned());

        for id in ids {
            let unit = self.unit(Who::Part(id), folder, format!("{ITEM_TAG} {id:03}"), item_label(sources, id));

            self.listed(unit, Pane::Figures(figures::Subject::DojoChests), CHEST_TABLE, Source::Variants);
        }
    }

    fn catfruit(&mut self, parent: u32) {
        let sources = self.sources;

        let Some(bytes) = sources.vault.vfs.load(catfruit::TABLE) else {
            return;
        };

        let ids: Vec<u32> = common::scrub(&bytes).lines().filter_map(catfruit::keyed).collect();

        if ids.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), catfruit::TABLE.to_owned(), catfruit::FOLDER.to_owned());

        for id in ids {
            let unit = self.unit(Who::Part(id), folder, format!("{ITEM_TAG} {id:03}"), item_label(sources, id));

            self.listed(unit, Pane::Catfruit, catfruit::TABLE, Source::Variants);
        }
    }

    fn shrine(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        if captions::SHRINE.iter().all(|layout| copies(vfs, layout.base()).is_empty()) {
            return;
        }

        let folder = self.folder(Some(parent), SHRINE.to_owned(), SHRINE.to_owned());

        self.homes.insert(SHRINE, folder);

        for layout in captions::SHRINE {
            self.captions(folder, layout);
        }

        self.captions(folder, captions::Layout::ShrineMembers);
    }

    fn god(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let present = GOD_SHEETS.iter().any(|kind| !copies(vfs, kind.base()).is_empty())
            || GOD_CAPTIONS.iter().any(|layout| !copies(vfs, layout.base()).is_empty());

        if !present {
            return;
        }

        let folder = self.folder(Some(parent), CAT_GOD.to_owned(), CAT_GOD.to_owned());

        for kind in GOD_SHEETS {
            if copies(vfs, kind.base()).is_empty() {
                continue;
            }

            let unit = self.unit(Who::Global, folder, kind.base().to_owned(), kind.title().to_owned());

            self.languages(unit, Pane::Sheet(kind), kind.base());
        }

        for layout in GOD_CAPTIONS {
            self.captions(folder, layout);
        }
    }

    fn talent_names(&mut self, parent: u32) {
        let mut numbers: BTreeSet<String> = BTreeSet::new();

        for name in self.sources.vault.vfs.glob(TALENT_NAME_PREFIX).iter() {
            let Some((stem, extension)) = name.rsplit_once('.') else {
                continue;
            };
            let number: String = stem[TALENT_NAME_PREFIX.len()..].chars().take_while(char::is_ascii_digit).collect();

            if !number.is_empty() && extension.eq_ignore_ascii_case(PICTURE_EXTENSION) {
                numbers.insert(number);
            }
        }

        if numbers.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), TALENT_NAME_FOLDER.to_owned(), TALENT_NAME_FOLDER.to_owned());

        for number in numbers {
            let base = format!("{TALENT_NAME_PREFIX}{number}.{PICTURE_EXTENSION}");
            let unit = self.unit(Who::Global, folder, base.clone(), format!("{TALENT_NAME} {number}"));

            self.languages(unit, Pane::Artwork(Art::Image), &base);
        }
    }

    fn talks(&mut self, parent: u32) {
        let mut scripts: BTreeSet<String> = BTreeSet::new();

        for name in self.sources.vault.vfs.glob(captions::TALK_PREFIX).iter() {
            let Some((stem, extension)) = name.rsplit_once('.') else {
                continue;
            };
            let number: String = stem[captions::TALK_PREFIX.len()..].chars().take_while(char::is_ascii_digit).collect();

            if !number.is_empty() && extension.eq_ignore_ascii_case(TALK_EXTENSION) {
                scripts.insert(number);
            }
        }

        if scripts.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), TALK_FOLDER.to_owned(), TALK_FOLDER.to_owned());

        self.homes.insert(TALK_FOLDER, folder);

        for number in scripts {
            let base = format!("{}{number}.{TALK_EXTENSION}", captions::TALK_PREFIX);

            self.captioned(folder, captions::Layout::Talks, &base, format!("{TALK_NAME} {number}"));
        }
    }

    fn captions(&mut self, parent: u32, layout: captions::Layout) {
        self.captioned(parent, layout, layout.base(), layout.title().to_owned());
    }

    fn captioned(&mut self, parent: u32, layout: captions::Layout, base: &str, title: String) {
        let vfs = &self.sources.vault.vfs;

        let Some((name, bytes)) =
            copies(vfs, base).into_iter().next().and_then(|name| vfs.load(&name).map(|bytes| (name, bytes)))
        else {
            return;
        };

        let delimiter = captions::delimiter_of(&name);

        let entries: Vec<(u32, String)> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .filter(|(at, line)| !line.trim().is_empty() && !(layout.headed() && *at == 0))
            .filter_map(|(at, line)| {
                let key = match layout.keyed() {
                    true => captions::keyed(line, delimiter)?,
                    false => u32::try_from(at).ok()?,
                };

                Some((key, layout.named(key, line, delimiter)))
            })
            .collect();

        if entries.is_empty() {
            return;
        }

        let single = entries.len() == 1;

        let folder = match single {
            true => parent,
            false => self.folder(Some(parent), base.to_owned(), title),
        };

        for (key, named) in entries {
            let (who, tag) = match (layout.keyed(), single) {
                (true, false) => (Who::Part(key), format!("{PART_TAG} {key:03}")),
                (false, false) => (Who::Line(key as usize), format!("{LINE_TAG} {key:03}")),
                (true, true) => (Who::Part(key), base.to_owned()),
                (false, true) => (Who::Line(key as usize), base.to_owned()),
            };

            let unit = self.unit(who, folder, tag, named);

            self.languages(unit, Pane::Captions(layout), base);
        }
    }

    fn cpu(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(CPU_TABLE) else {
            return;
        };

        let count = common::scrub(&bytes).lines().filter(|line| !line.trim().is_empty()).count();

        if count == 0 {
            return;
        }

        let folder = self.folder(Some(parent), CPU_TABLE.to_owned(), CPU_FOLDER.to_owned());

        for line in 0..count {
            let unit = self.unit(Who::Line(line), folder, format!("{CPU_LINE} {line}"), format!("{CPU_LINE} {line}"));

            self.listed(unit, Pane::Figures(figures::Subject::Cpu), CPU_TABLE, Source::Variants);
        }
    }

    fn zombies(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(ZOMBIE_TABLE) else {
            return;
        };

        let text = common::scrub(&bytes);
        let mut folder: Option<u32> = None;
        let mut tier = 0usize;
        let mut header = true;

        for (line, raw) in text.lines().enumerate() {
            let data = raw.split("//").next().unwrap_or_default();
            let cells: Vec<&str> = data.split(',').map(str::trim).filter(|cell| !cell.is_empty()).collect();

            if cells.len() < 2 {
                header = true;
                continue;
            }

            let holder = *folder.get_or_insert_with(|| self.folder(Some(parent), ZOMBIE_TABLE.to_owned(), ZOMBIE_FOLDER.to_owned()));

            let (subject, name) = match header {
                true => (figures::Subject::ZombieHeader, format!("{ZOMBIE_SETTINGS} {}", cells.first().copied().unwrap_or_default())),
                false => {
                    tier += 1;

                    (figures::Subject::ZombieTier, format!("{ZOMBIE_TIER} {tier}: {} {ZOMBIE_CLEARED}", cells.first().copied().unwrap_or_default()))
                }
            };

            header = false;

            let unit = self.unit(Who::Line(line), holder, format!("{LINE_TAG} {line:02}"), name);

            self.listed(unit, Pane::Figures(subject), ZOMBIE_TABLE, Source::Variants);
        }
    }

    fn gifts(&mut self, parent: u32) {
        let sources = self.sources;

        let Some(bytes) = sources.vault.vfs.load(gift::TABLE) else {
            return;
        };

        let text = common::scrub(&bytes);
        let ranks: Vec<(u32, &str)> = text.lines().filter_map(|line| Some((line.split(',').next()?.trim().parse().ok()?, line))).collect();

        if ranks.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), gift::TABLE.to_owned(), gift::FOLDER.to_owned());

        for (rank, line) in ranks {
            let first = gift::rewards(line).first().and_then(|(item, _)| u32::try_from(*item).ok()).map(|item| item_label(sources, item));

            let name = match first {
                Some(item) => format!("{} {rank}: {item}", gift::NOUN),
                None => format!("{} {rank}", gift::NOUN),
            };

            let unit = self.unit(Who::Part(rank), folder, format!("{} {rank:05}", gift::NOUN), name);

            self.listed(unit, Pane::Gift, gift::TABLE, Source::Variants);
        }
    }

    fn shop(&mut self, parent: u32) {
        let sources = self.sources;

        let Some(bytes) = sources.vault.vfs.load(SHOP_TABLE) else {
            return;
        };

        let slots: Vec<(u32, Option<u32>)> = common::scrub(&bytes)
            .lines()
            .filter_map(|line| {
                let mut cells = line.split('\t').map(str::trim);
                let slot = cells.next()?.parse().ok()?;

                Some((slot, cells.next().and_then(|cell| cell.parse().ok())))
            })
            .collect();

        if slots.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), SHOP_TABLE.to_owned(), ITEM_SHOP.to_owned());

        for (slot, item) in slots {
            let name = item.map_or_else(|| format!("{SHOP_NAME} {slot}"), |item| format!("{SHOP_NAME} {slot}: {}", item_label(sources, item)));
            let unit = self.unit(Who::Part(slot), folder, format!("{SHOP_NAME} {slot:02}"), name);

            self.listed(unit, Pane::Figures(figures::Subject::Shop), SHOP_TABLE, Source::Variants);
        }
    }

    fn recipes(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(RECIPE_TABLE) else {
            return;
        };

        let ids: Vec<u32> = common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect();

        if ids.is_empty() {
            return;
        }

        let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

        self.cat_base = Some(holder);

        let folder = self.folder(Some(holder), RECIPE_TABLE.to_owned(), RECIPES.to_owned());

        for id in ids {
            let unit = self.unit(Who::Part(id), folder, format!("{RECIPE_NAME} {id:02}"), format!("{RECIPE_NAME} {id}"));

            self.listed(unit, Pane::Figures(figures::Subject::Recipes), RECIPE_TABLE, Source::Variants);
        }
    }

    fn shakes(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(SHAKE_TABLE) else {
            return;
        };

        let ids: Vec<u32> = common::scrub(&bytes).lines().map_while(|line| line.split(',').next()?.trim().parse().ok()).collect();

        if ids.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), SHAKE_TABLE.to_owned(), SHAKES.to_owned());

        for id in ids {
            let unit = self.unit(Who::Part(id), folder, format!("{SHAKE_NAME} {id}"), format!("{SHAKE_NAME} {id}"));

            self.listed(unit, Pane::Figures(figures::Subject::Shakes), SHAKE_TABLE, Source::Variants);
        }
    }

    fn ex_lottery(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(EX_LOTTERY_TABLE) else {
            return;
        };

        let rows: Vec<(i64, i64)> = common::scrub(&bytes)
            .lines()
            .map_while(|line| {
                let mut cells = line.split(',').map(str::trim);

                Some((cells.next()?.parse().ok()?, cells.next()?.parse().ok()?))
            })
            .collect();

        if rows.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), EX_LOTTERY_TABLE.to_owned(), EX_LOTTERY.to_owned());

        for (line, (map, stage)) in rows.into_iter().enumerate() {
            let name = format!("{EX_GROUP} {line}: {EX_MAP} {map}, {EX_STAGE} {stage:02}");
            let unit = self.unit(Who::Line(line), folder, format!("{EX_GROUP} {line:03}"), name);

            self.listed(unit, Pane::Figures(figures::Subject::ExLottery), EX_LOTTERY_TABLE, Source::Variants);
        }
    }

    fn base(&mut self, parent: u32) {
        let sources = self.sources;
        let vfs = &sources.vault.vfs;
        let parts = Parts::load(vfs);

        let statused: Vec<u32> = vfs.load(CANNON_STATUS_TABLE).map_or_else(Vec::new, |bytes| {
            common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect()
        });

        let recipes = captions::Layout::Recipes;

        let mut captioned: BTreeMap<u32, String> = copies(vfs, recipes.base()).first().and_then(|name| vfs.load(name).map(|bytes| (name.clone(), bytes))).map_or_else(
            BTreeMap::new,
            |(name, bytes)| {
                let delimiter = captions::delimiter_of(&name);

                common::scrub(&bytes)
                    .lines()
                    .filter_map(|line| Some((captions::keyed(line, delimiter)?, line.split(delimiter).nth(1).map_or_else(String::new, |cell| cell.trim().to_owned()))))
                    .collect()
            },
        );

        for assembly in ASSEMBLIES {
            let kind = Ledger::Growth(assembly);

            let Some(text) = vfs.load(kind.base()).map(|bytes| common::scrub(&bytes)) else {
                continue;
            };

            let mut ids: Vec<u32> = text.lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect();

            ids.sort_unstable();
            ids.dedup();

            if ids.is_empty() {
                continue;
            }

            let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

            self.cat_base = Some(holder);

            let folder = self.folder(Some(holder), kind.base().to_owned(), kind.folder().to_owned());


            for id in ids {
                let part = i32::try_from(id).unwrap_or(i32::MAX);

                let name = match assembly {
                    Assembly::Cannon => sources.names.cannon(part).map_or_else(|| sandbox_base::name(part), str::to_owned),
                    Assembly::Foundation => parts.foundation_label(part, sources.names),
                    Assembly::Style => parts.style_label(part, sources.names),
                };

                let tag = format!("{PART_TAG} {id}");

                if assembly != Assembly::Cannon {
                    let unit = self.unit(Who::Part(id), folder, tag, name);

                    self.listed(unit, Pane::Ledger(kind), kind.base(), Source::Variants);

                    continue;
                }

                let home = self.folder(Some(folder), tag.clone(), name.clone());
                let unit = self.unit(Who::Part(id), home, tag, name);

                self.rows(unit, Pane::Ledger(kind), kind.base(), Source::Variants);

                if statused.contains(&id) {
                    self.rows(unit, Pane::Figures(figures::Subject::CannonStatus), CANNON_STATUS_TABLE, Source::Variants);
                }

                if captioned.remove(&id).is_some() {
                    self.rows(unit, Pane::Captions(recipes), recipes.base(), Source::Languages);
                }
            }
        }

        self.castle_growth(parent);
        self.development(parent);

        for (id, named) in captioned {
            let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

            self.cat_base = Some(holder);

            let part = i32::try_from(id).unwrap_or(i32::MAX);
            let name = match named.is_empty() {
                true => sources.names.cannon(part).map_or_else(|| sandbox_base::name(part), str::to_owned),
                false => named,
            };
            let unit = self.unit(Who::Part(id), holder, format!("{PART_TAG} {id}"), name);

            self.languages(unit, Pane::Captions(recipes), recipes.base());
        }
    }

    fn development(&mut self, parent: u32) {
        let sources = self.sources;
        let vfs = &sources.vault.vfs;
        let cannon = |id: u32| {
            let part = i32::try_from(id).unwrap_or(i32::MAX);

            sources.names.cannon(part).map_or_else(|| sandbox_base::name(part), str::to_owned)
        };
        let numbered = |text: &str| -> Vec<usize> {
            text.lines().enumerate().filter(|(_, line)| line.split(',').next().is_some_and(|cell| cell.trim().parse::<i64>().is_ok())).map(|(at, _)| at).collect()
        };
        let mut costs: Option<u32> = None;

        for (prefix, part) in DEVELOPMENT_COST_SETS {
            let tables: BTreeSet<String> = vfs.glob(prefix).iter().filter(|name| name.ends_with(".csv")).map(|name| name.to_string()).collect();

            for table in tables {
                let Some(id) = table.get(prefix.len()..).and_then(|rest| rest.trim_end_matches(".csv").parse::<u32>().ok()) else {
                    continue;
                };
                let lines = vfs.load(&table).map_or_else(Vec::new, |bytes| numbered(&common::scrub(&bytes)));

                if lines.is_empty() {
                    continue;
                }

                let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

                self.cat_base = Some(holder);

                let root = *costs.get_or_insert_with(|| self.folder(Some(holder), DEVELOPMENT_COST_FOLDER.to_owned(), DEVELOPMENT_COST_FOLDER.to_owned()));
                let folder = self.folder(Some(root), table.clone(), format!("{}: {part}", cannon(id)));

                for (level, line) in lines.into_iter().enumerate() {
                    let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), format!("{DEVELOPMENT_COST_NAME} {level}"));

                    self.listed(unit, Pane::Figures(figures::Subject::DevelopmentCosts), &table, Source::Variants);
                }
            }
        }

        if let Some(text) = vfs.load(DEVELOPMENT_TIER_TABLE).map(|bytes| common::scrub(&bytes)) {
            let tiers: Vec<(usize, String)> = text
                .lines()
                .enumerate()
                .filter_map(|(at, line)| {
                    let mut cells = line.split(',').map(|cell| cell.trim().parse::<u32>().ok());
                    let id = cells.next()??;
                    let part = cells.next()?.and_then(|kind| DEVELOPMENT_PARTS.get(kind as usize)).copied().unwrap_or(DEVELOPMENT_TIER_NAME);
                    let cap = cells.nth(2)?.map_or_else(String::new, |cap| cap.to_string());

                    Some((at, format!("{}: {part} to {CASTLE_LEVEL_LABEL} {cap}", cannon(id))))
                })
                .collect();

            if !tiers.is_empty() {
                let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

                self.cat_base = Some(holder);

                let folder = self.folder(Some(holder), DEVELOPMENT_TIER_TABLE.to_owned(), DEVELOPMENT_TIER_FOLDER.to_owned());

                for (line, named) in tiers {
                    let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), named);

                    self.listed(unit, Pane::Figures(figures::Subject::DevelopmentTiers), DEVELOPMENT_TIER_TABLE, Source::Variants);
                }
            }
        }

        if vfs.find(ENGINEER_CAP_TABLE).is_some() {
            let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

            self.cat_base = Some(holder);

            let unit = self.unit(Who::Line(0), holder, ENGINEER_CAP_TABLE.to_owned(), ENGINEER_CAP_NAME.to_owned());

            self.listed(unit, Pane::Figures(figures::Subject::EngineerCap), ENGINEER_CAP_TABLE, Source::Variants);
        }

        let mixes: Vec<u32> = vfs.load(MATERIAL_MIX_TABLE).map_or_else(Vec::new, |bytes| {
            common::scrub(&bytes).lines().filter_map(|line| line.split(',').next()?.trim().parse().ok()).collect()
        });

        if mixes.is_empty() {
            return;
        }

        let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

        self.cat_base = Some(holder);

        let folder = self.folder(Some(holder), MATERIAL_MIX_TABLE.to_owned(), MATERIAL_MIX_FOLDER.to_owned());

        for id in mixes {
            let unit = self.unit(Who::Part(id), folder, format!("{ITEM_TAG} {id:03}"), item_label(sources, id));

            self.listed(unit, Pane::Figures(figures::Subject::MaterialMixes), MATERIAL_MIX_TABLE, Source::Variants);
        }
    }

    fn castle_growth(&mut self, parent: u32) {
        let Some(bytes) = self.sources.vault.vfs.load(CASTLE_GROWTH_TABLE) else {
            return;
        };

        let brackets: Vec<(usize, String)> = common::scrub(&bytes)
            .lines()
            .enumerate()
            .filter_map(|(at, line)| line.split(',').next().and_then(|cell| cell.trim().parse::<i64>().ok()).map(|level| (at, level.to_string())))
            .collect();

        if brackets.is_empty() {
            return;
        }

        let holder = self.cat_base.unwrap_or_else(|| self.folder(Some(parent), CAT_BASE.to_owned(), CAT_BASE.to_owned()));

        self.cat_base = Some(holder);

        let folder = self.folder(Some(holder), CASTLE_GROWTH_TABLE.to_owned(), CASTLE_GROWTH_FOLDER.to_owned());

        for (position, (line, level)) in brackets.into_iter().enumerate() {
            let named = format!("{CASTLE_BRACKET_NAME} {}: {CASTLE_LEVEL_LABEL} {level}", position + 1);
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line:02}"), named);

            self.listed(unit, Pane::Figures(figures::Subject::CastleGrowth), CASTLE_GROWTH_TABLE, Source::Variants);
        }
    }

    fn upgrades(&mut self, parent: u32) {
        if self.sources.vault.vfs.find(UPGRADE_TABLE).is_none() {
            return;
        }

        let folder = self.folder(Some(parent), UPGRADE_TABLE.to_owned(), UPGRADES.to_owned());

        for (line, name) in UPGRADE_NAMES.iter().enumerate() {
            let unit = self.unit(Who::Line(line), folder, format!("{LINE_TAG} {line}"), (*name).to_owned());

            self.listed(unit, Pane::Figures(figures::Subject::Upgrade), UPGRADE_TABLE, Source::Variants);
        }
    }

    fn backgrounds(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;

        let table = vfs
            .find(background::TABLE)
            .and_then(|path| vfs.read(&path).ok())
            .and_then(|bytes| Background::parse(bytes.as_ref(), None).ok());

        let mut folder: Option<u32> = None;

        if let Some(table) = &table {
            let held = self.folder(Some(parent), background::TABLE.to_owned(), BACKGROUNDS.to_owned());

            folder = Some(held);

            for id in background::Catalog::from_table(table).ids {
                let Some(entry) = table.entries.iter().find(|entry| entry.id == id) else {
                    continue;
                };

                let tag = format!("{id:03}");
                let named = format!("{BACKGROUND_NAME} {id:03}");
                let home = self.folder(Some(held), tag.clone(), named.clone());
                let unit = self.unit(Who::Background(id), home, tag, named);

                self.rows(unit, Pane::Figures(figures::Subject::Background), background::TABLE, Source::Variants);
                self.rows(unit, Pane::Artwork(Art::Background), &backdrop::sheet_name(entry), Source::Languages);
                self.rows(unit, Pane::Cuts, &backdrop::cuts_name(entry), Source::Languages);
            }
        }

        let strays = self.strays(BACKGROUND_PREFIX, backdrop_stem);

        if strays.is_empty() {
            return;
        }

        let held = folder.unwrap_or_else(|| self.folder(Some(parent), background::TABLE.to_owned(), BACKGROUNDS.to_owned()));

        self.shelve(held, BACKGROUND_STRAYS, strays);
    }

    fn strays(&self, prefix: &str, fits: fn(&str) -> bool) -> Vec<(String, Pane)> {
        let mut found: Vec<(String, Pane)> = self
            .sources
            .vault
            .vfs
            .glob(prefix)
            .iter()
            .filter_map(|name| {
                let (stem, extension) = name.rsplit_once('.')?;
                let pane = match extension.to_ascii_lowercase().as_str() {
                    PICTURE_EXTENSION => Pane::Artwork(Art::Image),
                    CUT_EXTENSION => Pane::Cuts,
                    _ => return None,
                };

                (fits(stem) && !self.files_at.contains_key(name.as_ref()) && !rigged(&self.rigs, name)).then(|| (name.to_string(), pane))
            })
            .collect();

        found.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        found
    }

    fn shelve(&mut self, parent: u32, heading: &str, names: Vec<(String, Pane)>) {
        let folder = self.folder(Some(parent), heading.to_owned(), heading.to_owned());

        for (name, pane) in names {
            let unit = self.unit(Who::Global, folder, name.clone(), name.clone());

            self.listed(unit, pane, &name, Source::Single);
        }
    }

    fn map_sheets(&mut self, parent: u32) {
        let vfs = &self.sources.vault.vfs;
        let mut sets: BTreeMap<u32, Vec<(String, String, Pane)>> = BTreeMap::new();

        for name in vfs.glob(MAP_SHEET_PREFIX).iter() {
            let Some((set, sheet, picture)) = sheet_of(name) else {
                continue;
            };

            let (pane, named) = match picture {
                true => (Pane::Artwork(Art::Image), format!("{MAP_SHEET} {sheet}")),
                false => (Pane::Cuts, format!("{MAP_SHEET} {sheet} {}", Pane::Cuts.title())),
            };

            sets.entry(set).or_default().push((name.to_string(), named, pane));
        }

        let rows: Vec<(u32, u32)> = vfs.load(MAP_SHEET_TABLE).map_or_else(Vec::new, |bytes| {
            common::scrub(&bytes)
                .lines()
                .filter_map(|line| {
                    let mut cells = line.split('\t');
                    let id: u32 = cells.next()?.trim().parse().ok()?;
                    let set = cells.next().and_then(|cell| cell.trim().parse().ok()).unwrap_or(id);

                    Some((id, set))
                })
                .collect()
        });

        if rows.is_empty() && sets.is_empty() {
            return;
        }

        for sheets in sets.values_mut() {
            sheets.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        }

        let folder = self.folder(Some(parent), MAP_SHEET_TABLE.to_owned(), MAP_SHEET_FOLDER.to_owned());
        let mut placed: BTreeSet<u32> = BTreeSet::new();

        for (id, set) in rows {
            let tag = format!("{MAP_SHEET_NAME} {id:03}");
            let named = format!("{MAP_SHEET_NAME} {id}");
            let home = self.folder(Some(folder), tag.clone(), named.clone());
            let unit = self.unit(Who::Part(id), home, tag, named);

            self.rows(unit, Pane::Figures(figures::Subject::MapSheets), MAP_SHEET_TABLE, Source::Variants);
            self.sheets_of(home, sets.get(&set));
            placed.insert(set);
        }

        for (set, sheets) in sets.iter().filter(|(set, _)| !placed.contains(set)) {
            let named = format!("{MAP_SHEET_SET} {set:03}");
            let home = self.folder(Some(folder), named.clone(), named);

            self.sheets_of(home, Some(sheets));
        }
    }

    fn sheets_of(&mut self, folder: u32, sheets: Option<&Vec<(String, String, Pane)>>) {
        for (name, named, pane) in sheets.into_iter().flatten() {
            let unit = self.unit(Who::Global, folder, name.clone(), named.clone());

            self.listed(unit, *pane, name, Source::Single);
        }
    }

    fn items(&mut self, parent: u32) {
        let sources = self.sources;
        let catalog = sources.vault.vds.items.catalog(&sources.vault.vfs);

        let mut ids: Vec<u32> = catalog.iter().filter_map(|row| u32::try_from(row.stage_drop_item_id).ok()).collect();

        ids.sort_unstable();
        ids.dedup();

        let attributes = [
            (Pane::Figures(figures::Subject::ItemBuy), stage_files::GATYA_ITEM_BUY, Source::Variants, ITEM_CATALOG),
            (Pane::Prose(prose::Subject::ItemName), stage_files::GATYA_ITEM_NAME, Source::Exception, ITEM_TEXT),
        ];

        for (pane, base, source, title) in attributes {
            let folder = self.folder(Some(parent), base.to_owned(), title.to_owned());

            for id in &ids {
                let unit = self.unit(Who::Item(*id), folder, format!("{ITEM_TAG} {id:03}"), item_label(sources, *id));

                self.listed(unit, pane, base, source);
            }
        }

        let vfs = &sources.vault.vfs;

        let icons: Vec<(u32, String)> = ids
            .iter()
            .filter_map(|id| Some((*id, stage_files::gatya_item_img(sources.vault.vds.items.icon_index(vfs, *id)?))))
            .filter(|(_, file)| vfs.find(file).is_some())
            .collect();

        if icons.is_empty() {
            return;
        }

        let folder = self.folder(Some(parent), ITEM_ICON_RAW.to_owned(), ITEM_ICONS.to_owned());

        for (id, file) in icons {
            let unit = self.unit(Who::Item(id), folder, format!("{ITEM_TAG} {id:03}"), item_label(sources, id));

            self.listed(unit, Pane::Artwork(Art::ItemIcon), &file, Source::Languages);
        }
    }

    fn abilities(&mut self, parent: u32, faction: Faction) {
        let names = &self.sources.names;
        let file = glossary_file(faction);

        let pane = match faction {
            Faction::Cat => Pane::Prose(prose::Subject::CatGlossary),
            Faction::Enemy => Pane::Prose(prose::Subject::EnemyGlossary),
        };

        let folder = self.folder(Some(parent), file.to_owned(), pane.title().to_owned());

        for ability in REGISTRY {
            let Some(line) = names.glossary_line(ability.identity, faction).filter(|line| *line > 0) else {
                continue;
            };

            let named = names.ability(ability.identity, faction).to_owned();
            let who = Who::Ability(ability.identity, faction);
            let unit = self.unit(who, folder, format!("{LINE_TAG} {line}"), named);

            self.listed(unit, pane, file, Source::Exception);
        }
    }

    fn folder(&mut self, parent: Option<u32>, raw: String, resolved: String) -> u32 {
        let at = u32::try_from(self.catalog.folders.len()).unwrap_or(u32::MAX);

        self.catalog.folders.push(Folder {
            raw_key: raw.to_lowercase().into(),
            resolved_key: resolved.to_lowercase().into(),
            raw: raw.into(),
            resolved: resolved.into(),
            parent,
            folders: Vec::new(),
            leaves: Vec::new(),
        });

        match parent.and_then(|parent| self.catalog.folders.get_mut(parent as usize)) {
            Some(held) => held.folders.push(at),
            None => self.catalog.roots.push(at),
        }

        at
    }

    fn unit(&mut self, who: Who, folder: u32, tag: String, name: String) -> u32 {
        let at = u32::try_from(self.catalog.units.len()).unwrap_or(u32::MAX);

        self.catalog.units.push(Unit {
            who,
            folder,
            tag_key: tag.to_lowercase().into(),
            name_key: name.to_lowercase().into(),
            tag: tag.into(),
            named: name.into(),
        });

        at
    }

    fn rows(&mut self, unit: u32, pane: Pane, base: &str, source: Source) {
        self.placed(unit, pane, base, source, pane.title(), Face::File);
    }

    fn headed(&mut self, unit: u32, pane: Pane, base: &str, heading: &'static str) {
        self.placed(unit, pane, base, Source::Variants, heading, Face::File);
    }

    fn listed(&mut self, unit: u32, pane: Pane, base: &str, source: Source) {
        self.placed(unit, pane, base, source, pane.title(), Face::Unit);
    }

    fn placed(&mut self, unit: u32, pane: Pane, base: &str, source: Source, heading: &'static str, face: Face) {
        let Some(home) = self.catalog.units.get(unit as usize).map(|held| held.folder) else {
            return;
        };

        let set = self.set(pane, base, source, heading);

        let folder = match set.len() > 1 {
            true => self.folder(Some(home), base.to_owned(), heading.to_owned()),
            false => home,
        };

        self.attach(unit, pane, set, folder, face);
    }

    fn languages(&mut self, unit: u32, pane: Pane, base: &str) {
        let Some((home, tag, named)) = self.catalog.units.get(unit as usize).map(|held| (held.folder, held.tag.to_string(), held.named.to_string())) else {
            return;
        };

        let set = self.set(pane, base, Source::Languages, pane.title());

        let (folder, face) = match set.len() > 1 {
            true => (self.folder(Some(home), tag, named), Face::File),
            false => (home, Face::Unit),
        };

        self.attach(unit, pane, set, folder, face);
    }

    fn attach(&mut self, unit: u32, pane: Pane, set: Range<usize>, folder: u32, face: Face) {
        for at in set {
            let Some(&(file, title)) = self.arena.get(at) else {
                continue;
            };

            let next = u32::try_from(self.catalog.rows.len()).unwrap_or(u32::MAX);

            self.catalog.rows.push(Row { pane, unit, file, title, folder, face });

            if let Some(held) = self.catalog.folders.get_mut(folder as usize) {
                held.leaves.push(next);
            }
        }
    }

    fn fresh(&mut self, unit: u32, pane: Pane, base: &str, label: &str) {
        let Some(folder) = self.catalog.units.get(unit as usize).map(|held| held.folder) else {
            return;
        };

        let Some(&(file, _)) =
            self.set(pane, base, Source::Variants, pane.title()).next().and_then(|at| self.arena.get(at))
        else {
            return;
        };

        let title = self.title(label);
        let next = u32::try_from(self.catalog.rows.len()).unwrap_or(u32::MAX);

        self.catalog.rows.push(Row { pane, unit, file, title, folder, face: Face::Title });

        if let Some(held) = self.catalog.folders.get_mut(folder as usize) {
            held.leaves.push(next);
        }
    }

    fn set(&mut self, pane: Pane, base: &str, source: Source, heading: &'static str) -> Range<usize> {
        if let Some(range) = self.sets.get(&(pane, heading)).and_then(|held| held.get(base)) {
            return range.clone();
        }

        let vfs = &self.sources.vault.vfs;

        let named: Vec<(String, String)> = match source {
            Source::Single => match vfs.rooted(architecture::GAME, base) {
                Some(_) => vec![(base.to_owned(), heading.to_owned())],
                None => Vec::new(),
            },
            Source::Exception => match vfs.find(base) {
                Some(_) => vec![(internal_name(self.sources, &visible_name(self.sources, base)), heading.to_owned())],
                None => Vec::new(),
            },
            Source::Image => vec![(internal_name(self.sources, &visible_name(self.sources, base)), heading.to_owned())],
            Source::Variants | Source::Languages => {
                let names = copies(vfs, base);
                let shared = names.len() > 1;

                names
                    .into_iter()
                    .map(|name| {
                        let title = match shared {
                            true => registry::language(&name).map_or_else(|| name.clone(), str::to_owned),
                            false => heading.to_owned(),
                        };

                        (name, title)
                    })
                    .collect()
            }
        };

        let regional: Vec<String> = match (source, named.first()) {
            (Source::Image | Source::Exception, Some((internal, _))) => {
                copies(vfs, internal)
            }
            _ => Vec::new(),
        };

        let start = self.arena.len();

        for (name, title) in named {
            let file = self.file(&name);
            let title = self.title(&title);

            self.arena.push((file, title));
        }

        if let Some(&(file, _)) = self.arena.get(start) {
            for name in regional {
                self.files_at.entry(name.into()).or_insert(file);
            }
        }

        let range = start..self.arena.len();

        self.sets.entry((pane, heading)).or_default().insert(base.into(), range.clone());

        range
    }

    fn file(&mut self, name: &str) -> u32 {
        if let Some(at) = self.files_at.get(name) {
            return *at;
        }

        let at = u32::try_from(self.catalog.files.len()).unwrap_or(u32::MAX);

        self.catalog.files.push(name.into());
        self.catalog.file_keys.push(name.to_lowercase().into());
        self.files_at.insert(name.into(), at);

        at
    }

    fn title(&mut self, title: &str) -> u32 {
        if let Some(at) = self.titles_at.get(title) {
            return *at;
        }

        let at = u32::try_from(self.catalog.titles.len()).unwrap_or(u32::MAX);

        self.catalog.titles.push(title.into());
        self.catalog.title_keys.push(title.to_lowercase().into());
        self.titles_at.insert(title.into(), at);

        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_images_group_by_the_name_before_their_number() {
        assert_eq!(family("img015_ja.png"), "img");
        assert_eq!(family("gatya_bnr_012.png"), "gatya_bnr");
        assert_eq!(family("medal.png"), "medal");
        assert_eq!(family("000_a.png"), LOOSE_FAMILY);
    }

    #[test]
    fn a_cut_list_with_a_model_beside_it_is_left_to_the_animation_editor() {
        let rigs = ["scratch_000.mamodel".to_owned(), "slot_000_retry.mamodel".to_owned()];

        assert!(rigged(&rigs, "scratch_000.imgcut"));
        assert!(rigged(&rigs, "slot_000.imgcut"));
        assert!(rigged(&rigs, "scratch_000.png"));
        assert!(!rigged(&rigs, "img015.imgcut"));
        assert!(!rigged(&rigs, "slot_001.imgcut"));
        // a name that only begins another rig's name is not that rig's sheet
        assert!(!rigged(&rigs, "s.png"));
        assert!(!rigged(&rigs, "slot_00.png"));
    }

    #[test]
    fn unit_images_are_told_apart_by_their_shape() {
        assert!(matches!(unit_image("uni123_f00.png"), Some((123, 0, Art::DeployIcon))));
        assert!(matches!(unit_image("uni123_c00_ko.png"), Some((123, 1, Art::DeployIcon))));
        assert!(matches!(unit_image("udi123_u.png"), Some((123, 3, Art::Banner))));

        // Eggs, special banners and loose art are not a unit's form images.
        assert!(unit_image("uni003_m00.png").is_none() && unit_image("udi003_sg_en.png").is_none());
        assert!(unit_image("uni.png").is_none() && unit_image("udi123_f.imgcut").is_none());

        assert_eq!(egg_image("uni003_m00.png"), Some(3));
        assert_eq!(egg_image("udi003_m01.png"), Some(3));
        assert_eq!(egg_image("uni003_f00.png"), None);
    }

    #[test]
    fn stage_images_are_told_apart_by_their_shape() {
        assert_eq!(sheet_of("map003_01.png"), Some((3, "01", true)));
        assert_eq!(sheet_of("map003_01.imgcut"), Some((3, "01", false)));
        assert_eq!(sheet_of("mapicon.png"), None);
        assert_eq!(sheet_of("mapsn003_00_v_en.png"), None);

        assert!(backdrop_stem("bg012") && backdrop_stem("bgEffect_064") && backdrop_stem("bgEffect_010_00"));
        assert!(!backdrop_stem("bg") && !backdrop_stem("bgm_01x"));

        // Stage name plates share the castle prefixes and must not be swept up with them.
        assert!(castle_stem("rc187") && castle_stem("ec003_en") && castle_stem("castle_wc00"));
        assert!(!castle_stem("ec000_n_en") && !castle_stem("scratch_00"));
    }

    #[test]
    fn a_rating_row_is_found_by_its_unit_and_form() {
        let held = rated(b"0,1,1000\r\n0,2,900\n7,3,400\n7,3,1\n8,0,5\n//unit,form,rating\n9,1,5\n");

        assert_eq!(held.get(&(0, 0)), Some(&0));
        assert_eq!(held.get(&(0, 1)), Some(&1));
        // forms count from 1, a repeated pair keeps its last row, and the table ends at the first row that is not a number
        assert_eq!(held.get(&(7, 2)), Some(&3));
        assert_eq!(held.len(), 3);
    }

    fn folder(raw: &str, resolved: &str, parent: Option<u32>, folders: Vec<u32>, leaves: Vec<u32>) -> Folder {
        Folder {
            raw: raw.into(),
            resolved: resolved.into(),
            raw_key: raw.to_lowercase().into(),
            resolved_key: resolved.to_lowercase().into(),
            parent,
            folders,
            leaves,
        }
    }

    fn unit(id: u32, folder: u32, name: &str) -> Unit {
        Unit {
            who: Who::Cat { id, form: 0 },
            folder,
            tag: format!("{id:03}_f").into(),
            named: name.into(),
            tag_key: format!("{id:03}_f").into(),
            name_key: name.to_lowercase().into(),
        }
    }

    fn row(unit: u32, file: u32, folder: u32) -> Row {
        Row { pane: Pane::Figures(figures::Subject::Buy), unit, file, title: 0, folder, face: Face::File }
    }

    // Cats > 000 > Cat           unitbuy.csv, unit001.csv
    //      > 002 > Tank Cat      unitbuy.csv
    fn roster() -> Catalog {
        Catalog {
            units: vec![unit(0, 2, "Cat"), unit(2, 4, "Tank Cat")],
            folders: vec![
                folder("Cats", "Cats", None, vec![1, 3], Vec::new()),
                folder("000", "Cat", Some(0), vec![2], Vec::new()),
                folder("000_f", "Cat", Some(1), Vec::new(), vec![0, 1]),
                folder("002", "Tank Cat", Some(0), vec![4], Vec::new()),
                folder("002_f", "Tank Cat", Some(3), Vec::new(), vec![2]),
            ],
            roots: vec![0],
            files: vec!["unitbuy.csv".into(), "unit001.csv".into()],
            file_keys: vec!["unitbuy.csv".into(), "unit001.csv".into()],
            titles: vec!["Progression".into()],
            title_keys: vec!["progression".into()],
            rows: vec![row(0, 0, 2), row(0, 1, 2), row(1, 0, 4)],
        }
    }

    fn shown(catalog: &Catalog, query: &str) -> Vec<Entry> {
        let filter = catalog.filter(query);
        let searching = filter.searching();

        catalog.lines(&filter, |_| searching).into_iter().map(|line| line.entry).collect()
    }

    #[test]
    fn a_folder_name_narrows_the_files_beneath_it() {
        // "tank" only lives on the folder, "unitbuy" only on the file
        assert_eq!(
            shown(&roster(), "tank unitbuy"),
            vec![Entry::Folder(0), Entry::Folder(3), Entry::Folder(4), Entry::Leaf(2)],
        );
    }

    #[test]
    fn a_rename_keeps_your_place_in_the_tree() {
        let before = roster();
        let mut after = roster();

        // renaming only touches the resolved side, never the ids the place is kept by
        after.folders[3].resolved = "The Grim Reaper".into();
        after.folders[3].resolved_key = "the grim reaper".into();

        let folders = after.folder_ids();
        let lead = before.lead(2).expect("leaf 2 exists in the fixture");

        assert_eq!(folders.get(&before.folder_key(4).unwrap_or_default()), Some(&4));
        assert_eq!(after.follow(&lead, &folders), Some(2));
        assert_eq!(shown(&after, "grim unitbuy"), vec![Entry::Folder(0), Entry::Folder(3), Entry::Folder(4), Entry::Leaf(2)]);
    }

    #[test]
    fn browsing_starts_shut_and_a_search_opens_every_path_to_a_hit() {
        assert_eq!(shown(&roster(), ""), vec![Entry::Folder(0)]);

        assert_eq!(
            shown(&roster(), "(unit001"),
            vec![Entry::Folder(0), Entry::Folder(1), Entry::Folder(2), Entry::Leaf(1)],
        );

        assert!(shown(&roster(), "nothing").is_empty(), "a search with no hits leaves no empty folders behind");
    }
}
