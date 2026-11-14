use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, responsive, row, scrollable, space, text, text_input, toggler, Column, Id};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::cat::AbilityData;
use foundry::common::formats::json::{self, Record, Value};
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::common::row_window::{self, RowWindow};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader, UPGRADE_NAMES, UPGRADE_TABLE};

const SIZE: Size = Size::new(460.0, 560.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Preset, SIZE);

pub(super) const TITLE: &str = "Fixed Lineup";
pub(super) const FOLDER: &str = "Lineup Presets";

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const UNIT_GAP: f32 = 10.0;
const HEADING_SIZE: f32 = 14.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const NUMBER_WIDTH: f32 = 56.0;
const PICK_WIDTH: f32 = 130.0;
const RAW_WIDTH: f32 = 150.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;
const LINE_ROW: f32 = 30.0;


const SYNC_LABEL: &str = "Sync With \"game\"";
const SEARCH_HINT: &str = "Search Field...";
const EMPTY_LABEL: &str = "No fields match";
const UNREADABLE_LABEL: &str = "This preset could not be read";
const ADD_LABEL: &str = "Add Cat";
const ADD_HINT: &str = "Cat ID";
const REMOVE_LABEL: &str = "Remove";
const SAVED_LABEL: &str = "From Save";
const MISSING_LABEL: &str = "Not in this preset yet, editing it adds it";
const WIPE_LABEL: &str = "Chapters it leaves out start empty";
const LINEUP_HEADING: &str = "Lineup";
const UPGRADES_HEADING: &str = "Power Ups";
const CANNONS_HEADING: &str = "Cannon Levels";
const TREASURES_HEADING: &str = "Treasures";
const LEVEL_MARK: &str = "Lv";
const PLUS_MARK: &str = "+";
const UNKNOWN_CAT: &str = "Unknown Cat";
const CANNON_NAME: &str = "Cannon";
const LIST_JOINT: &str = ", ";
const PATH_JOINT: &str = "/";

const LINEUP_SLOTS: usize = 10;
const FULL_SET: i64 = 48;
const GRADE_CELLS: usize = 3;
const NEW_LEVEL: i64 = 30;
const NEW_FORM: i64 = 1;

const CHARA: [&str; 2] = ["chara", "data"];
const SLOT: [&str; 2] = ["slot", "data"];
const ABILITY: [&str; 2] = ["ability", "data"];
const CANNON: [&str; 2] = ["cannon", "data"];
const TREASURE: [&str; 2] = ["treasure", "data"];
const DEFAULTS: [&str; 2] = ["treasure", "defaultData"];
const TREASURE_ROOT: [&str; 1] = ["treasure"];
const DEFAULT_KEY: &str = "defaultData";

const EVOLUTION: &str = "evolution";
const LEVEL: &str = "level";
const PLUS: &str = "plus";
const REMOVE: &str = "remove";
const LINEUP: &str = "chara";
const SLOT_CANNON: &str = "cannon";
const COUNT: &str = "count";
const NONE: &str = "none";

const FORMS: [&str; 4] = ["Normal", "Evolved", "True", "Ultra"];


const CHAPTERS: [(&str, &str); 9] = [
    ("0", "EoC Ch. 1"),
    ("1", "EoC Ch. 2"),
    ("2", "EoC Ch. 3"),
    ("4", "ItF Ch. 1"),
    ("5", "ItF Ch. 2"),
    ("6", "ItF Ch. 3"),
    ("7", "CotC Ch. 1"),
    ("8", "CotC Ch. 2"),
    ("9", "CotC Ch. 3"),
];

const GRADES: [&str; 4] = ["None", "Inferior", "Normal", "Superior"];

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Member {
    pub(crate) id: u32,
    pub(crate) forms: Vec<String>,
    pub(crate) caps: Option<(i64, i64)>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    roster: Vec<Member>,
    cannons: Vec<String>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) roster: Vec<Member>,
    pub(super) cannons: Vec<String>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, mut roster, cannons } = seed;

    roster.sort_unstable_by_key(|member| member.id);

    Plan { label, file, game, target_mod, values, roster, cannons }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn member(&self, id: u32) -> Option<&Member> {
        self.roster.binary_search_by_key(&id, |member| member.id).ok().and_then(|at| self.roster.get(at))
    }

    fn name(&self, id: u32) -> String {
        self.member(id)
            .and_then(|member| member.forms.first())
            .map_or_else(|| format!("{UNKNOWN_CAT} {id:03}"), |name| format!("{name} ({id:03})"))
    }

    fn cannon(&self, part: i64) -> String {
        usize::try_from(part)
            .ok()
            .and_then(|at| self.cannons.get(at))
            .map_or_else(|| format!("{CANNON_NAME} {part}"), String::clone)
    }
}

fn same(path: &[String], wanted: &[&str]) -> bool {
    path.len() == wanted.len() && path.iter().zip(wanted).all(|(held, want)| held == want)
}

fn child<'a>(path: &'a [String], parent: &[&str]) -> Option<&'a str> {
    let (last, head) = path.split_last()?;

    same(head, parent).then_some(last.as_str())
}

fn number(value: &Value) -> Option<i64> {
    match value {
        Value::Number(held) | Value::Text(held) => held.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Flag(held) => *held,
        Value::Number(held) => held.parse::<f64>().is_ok_and(|parsed| parsed != 0.0),
        Value::Text(held) => !held.is_empty(),
        Value::Numbers(_) | Value::Texts(_) | Value::Tokens(_) | Value::Other => false,
    }
}

fn owned(record: &Record) -> bool {
    !record.field(REMOVE).is_some_and(|field| truthy(&field.value))
}

fn grade(value: &Value) -> usize {
    let cells: Vec<i64> = match value {
        Value::Numbers(items) | Value::Texts(items) => items.iter().map(|item| item.trim().parse::<i64>().unwrap_or(0)).collect(),
        _ => Vec::new(),
    };

    if cells.len() < GRADE_CELLS {
        return 0;
    }

    (0..GRADE_CELLS).rev().find(|at| cells.get(*at) == Some(&FULL_SET)).map_or(0, |at| at + 1)
}

fn render(value: &Value, number: i64) -> String {
    match value {
        Value::Number(_) => number.to_string(),
        _ => json::quote(&number.to_string()),
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::Number(held) | Value::Text(held) => held.clone(),
        Value::Numbers(items) | Value::Texts(items) | Value::Tokens(items) => items.join(LIST_JOINT),
        Value::Flag(held) => held.to_string(),
        Value::Other => String::new(),
    }
}

fn digits(typed: &str) -> bool {
    typed.chars().all(|glyph| glyph.is_ascii_digit())
}

fn integer(typed: &str) -> Option<i64> {
    let body = typed.strip_prefix('-').unwrap_or(typed);

    (!body.is_empty() && digits(body)).then(|| typed.parse::<i64>().ok()).flatten()
}

fn compact(words: &[String]) -> String {
    let quoted: Vec<String> = words.iter().map(|word| json::quote(word)).collect();

    format!("[{}]", quoted.join(","))
}

fn fresh(level: i64) -> String {
    format!("{{{}:{},{}:{},{}:{}}}", json::quote(EVOLUTION), json::quote(&NEW_FORM.to_string()), json::quote(LEVEL), json::quote(&level.to_string()), json::quote(PLUS), json::quote("0"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spot {
    Level(u32),
    Plus(u32),
    Upgrade(String),
    Boost(String),
    Cannon(String),
    Adding,
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    text: String,
    records: Vec<Record>,
    lines: Vec<(usize, usize)>,
    labels: Vec<String>,
    inputs: Vec<String>,
    caps: Option<AbilityData>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let text = fs::read_to_string(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Preset editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;

        let caps = vfs.load(UPGRADE_TABLE).and_then(|bytes| {
            AbilityData::parse(bytes.as_ref(), None)
                .inspect_err(|err| warn!("Preset editor could not read the upgrade caps: {}", err))
                .ok()
        });

        let mut draft = Draft {
            plan,
            read_from,
            stamp,
            text,
            records: Vec::new(),
            lines: Vec::new(),
            labels: Vec::new(),
            inputs: Vec::new(),
            caps,
            failed: false,
        };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        self.records = json::records(&self.text);
        self.lines.clear();
        self.labels.clear();
        self.inputs.clear();

        for (at, record) in self.records.iter().enumerate() {
            for (slot, field) in record.fields.iter().enumerate() {
                self.lines.push((at, slot));
                self.labels.push(format!("{}{PATH_JOINT}{}", record.path.join(PATH_JOINT), field.key));
                self.inputs.push(shown(&field.value));
            }
        }
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn find(&self, path: &[&str]) -> Option<usize> {
        self.records.iter().position(|record| same(&record.path, path))
    }

    fn record(&self, path: &[&str]) -> Option<&Record> {
        self.find(path).and_then(|at| self.records.get(at))
    }

    fn read(&self, path: &[&str], key: &str) -> Option<i64> {
        self.record(path)?.field(key).and_then(|field| number(&field.value))
    }

    fn object(&self, path: &[&str]) -> Option<Range<usize>> {
        json::objects(&self.text).into_iter().find(|node| same(&node.path, path)).map(|node| node.span)
    }

    fn slot(&self) -> Option<usize> {
        self.records
            .iter()
            .enumerate()
            .filter_map(|(at, record)| child(&record.path, &SLOT).map(|key| (key, at)))
            .min_by(|(left, _), (right, _)| left.cmp(right))
            .map(|(_, at)| at)
    }

    fn lineup(&self) -> Vec<u32> {
        let field = self.slot().and_then(|at| self.records.get(at)).and_then(|record| record.field(LINEUP));

        match field.map(|held| &held.value) {
            Some(Value::Texts(items) | Value::Numbers(items)) => items.iter().filter_map(|item| item.trim().parse::<u32>().ok()).collect(),
            _ => Vec::new(),
        }
    }

    fn slot_cannon(&self) -> Option<i64> {
        self.records.get(self.slot()?)?.field(SLOT_CANNON).and_then(|field| number(&field.value))
    }

    fn unit(&self, id: u32) -> Option<&Record> {
        let key = id.to_string();

        self.record(&[CHARA[0], CHARA[1], key.as_str()]).filter(|record| owned(record))
    }

    fn wiped(&self) -> bool {
        self.record(&DEFAULTS).and_then(|record| record.field(NONE)).is_some_and(|field| truthy(&field.value))
    }

    fn grade_of(&self, chapter: &str) -> Option<usize> {
        self.record(&[TREASURE[0], TREASURE[1], chapter])?.field(COUNT).map(|field| grade(&field.value))
    }

    fn keyed(&self, parent: &[&str]) -> Vec<(i64, String)> {
        let mut held: Vec<(i64, String)> = self
            .records
            .iter()
            .filter_map(|record| child(&record.path, parent))
            .filter_map(|key| key.parse::<i64>().ok().map(|number| (number, key.to_owned())))
            .collect();

        held.sort_unstable_by_key(|(number, _)| *number);
        held
    }

    fn rewrite(&mut self, text: Option<String>) -> bool {
        match text {
            Some(text) if text != self.text => {
                self.text = text;
                self.index();

                true
            }
            _ => false,
        }
    }

    fn put_at(&mut self, at: usize, key: &str, value: i64) -> bool {
        let Some(record) = self.records.get(at) else {
            return false;
        };

        let text = match record.field(key) {
            Some(field) => json::replace(&self.text, &field.span, &render(&field.value, value)),
            None => json::insert(&self.text, &record.span, key, &json::quote(&value.to_string())),
        };

        self.rewrite(text)
    }

    fn put(&mut self, path: &[&str], key: &str, value: i64) -> bool {
        self.find(path).is_some_and(|at| self.put_at(at, key, value))
    }

    fn own(&mut self, id: u32) -> bool {
        if self.unit(id).is_some() {
            return false;
        }

        let key = id.to_string();
        let level = self.plan.member(id).and_then(|member| member.caps).map_or(NEW_LEVEL, |(base, _)| base.min(NEW_LEVEL));
        let entry = fresh(level);

        let text = match self.object(&[CHARA[0], CHARA[1], key.as_str()]) {
            Some(span) => json::replace(&self.text, &span, &entry),
            None => self.object(&CHARA).and_then(|span| json::insert(&self.text, &span, &key, &entry)),
        };

        self.rewrite(text)
    }

    fn put_unit(&mut self, id: u32, key: &str, value: i64) -> bool {
        let added = self.own(id);
        let index = id.to_string();

        self.put(&[CHARA[0], CHARA[1], index.as_str()], key, value) || added
    }

    fn put_lineup(&mut self, ids: &[u32]) -> bool {
        let Some(field) = self.slot().and_then(|at| self.records.get(at)).and_then(|record| record.field(LINEUP)) else {
            return false;
        };

        let rendered = match &field.value {
            Value::Numbers(items) if !items.is_empty() => {
                let numbers: Vec<i64> = ids.iter().map(|id| i64::from(*id)).collect();

                json::numbers(&self.text, &field.span, &numbers)
            }
            _ => {
                let words: Vec<String> = ids.iter().map(u32::to_string).collect();

                json::texts(&self.text, &field.span, &words)
            }
        };

        let text = json::replace(&self.text, &field.span, &rendered);

        self.rewrite(text)
    }

    fn set_cannon(&mut self, cannon: i64) -> bool {
        self.slot().is_some_and(|at| self.put_at(at, SLOT_CANNON, cannon))
    }

    fn grade(&mut self, chapter: &str, grade: usize) -> bool {
        let mut cells = [0i64; GRADE_CELLS];

        if let Some(cell) = grade.checked_sub(1).and_then(|at| cells.get_mut(at)) {
            *cell = FULL_SET;
        }

        let words: Vec<String> = cells.iter().map(i64::to_string).collect();
        let path = [TREASURE[0], TREASURE[1], chapter];

        let text = match self.record(&path) {
            Some(record) => match record.field(COUNT) {
                Some(field) => {
                    let rendered = match &field.value {
                        Value::Numbers(items) if !items.is_empty() => json::numbers(&self.text, &field.span, &cells),
                        _ => json::texts(&self.text, &field.span, &words),
                    };

                    json::replace(&self.text, &field.span, &rendered)
                }
                None => json::insert(&self.text, &record.span, COUNT, &compact(&words)),
            },
            None => {
                let entry = format!("{{{}:{}}}", json::quote(COUNT), compact(&words));

                match self.object(&path) {
                    Some(span) => json::replace(&self.text, &span, &entry),
                    None => self.object(&TREASURE).and_then(|span| json::insert(&self.text, &span, chapter, &entry)),
                }
            }
        };

        self.rewrite(text)
    }

    fn wipe(&mut self, on: bool) -> bool {
        let flag = on.to_string();

        let text = match self.record(&DEFAULTS) {
            Some(record) => match record.field(NONE) {
                Some(field) => json::replace(&self.text, &field.span, &flag),
                None => json::insert(&self.text, &record.span, NONE, &flag),
            },
            None => match (self.object(&DEFAULTS), self.object(&TREASURE_ROOT)) {
                (Some(span), _) => json::insert(&self.text, &span, NONE, &flag),
                (None, Some(span)) => json::insert(&self.text, &span, DEFAULT_KEY, &format!("{{{}:{flag}}}", json::quote(NONE))),
                (None, None) => None,
            },
        };

        self.rewrite(text)
    }

    fn bounds(&self, spot: &Spot) -> (i64, Option<i64>) {
        let upgrade = |kind: &str| kind.parse::<usize>().ok().and_then(|kind| self.caps.as_ref()?.caps(kind));

        match spot {
            Spot::Level(id) => (1, self.plan.member(*id).and_then(|member| member.caps).map(|(base, _)| base)),
            Spot::Plus(id) => (0, self.plan.member(*id).and_then(|member| member.caps).map(|(_, plus)| plus)),
            Spot::Upgrade(kind) => (1, upgrade(kind).map(|(level, _)| i64::from(level))),
            Spot::Boost(kind) => (0, upgrade(kind).map(|(_, plus)| i64::from(plus))),
            Spot::Cannon(_) => (1, None),
            Spot::Adding => (0, None),
        }
    }

    fn current(&self, spot: &Spot) -> Option<i64> {
        match spot {
            Spot::Level(id) => self.unit(*id)?.field(LEVEL).and_then(|field| number(&field.value)),
            Spot::Plus(id) => self.unit(*id)?.field(PLUS).and_then(|field| number(&field.value)),
            Spot::Upgrade(kind) => self.read(&[ABILITY[0], ABILITY[1], kind.as_str()], LEVEL),
            Spot::Boost(kind) => self.read(&[ABILITY[0], ABILITY[1], kind.as_str()], PLUS),
            Spot::Cannon(part) => self.read(&[CANNON[0], CANNON[1], part.as_str()], LEVEL),
            Spot::Adding => None,
        }
    }

    fn set(&mut self, spot: &Spot, value: i64) -> bool {
        match spot {
            Spot::Level(id) => self.put_unit(*id, LEVEL, value),
            Spot::Plus(id) => self.put_unit(*id, PLUS, value),
            Spot::Upgrade(kind) => self.put(&[ABILITY[0], ABILITY[1], kind.as_str()], LEVEL, value),
            Spot::Boost(kind) => self.put(&[ABILITY[0], ABILITY[1], kind.as_str()], PLUS, value),
            Spot::Cannon(part) => self.put(&[CANNON[0], CANNON[1], part.as_str()], LEVEL, value),
            Spot::Adding => false,
        }
    }

    fn add(&mut self, id: u32) -> bool {
        let mut lineup = self.lineup();

        if lineup.len() >= LINEUP_SLOTS || lineup.contains(&id) || self.plan.member(id).is_none() {
            return false;
        }

        lineup.push(id);

        let added = self.own(id);

        self.put_lineup(&lineup) || added
    }

    fn drop_unit(&mut self, id: u32) -> bool {
        let lineup: Vec<u32> = self.lineup().into_iter().filter(|held| *held != id).collect();

        self.put_lineup(&lineup)
    }

    fn edit(&mut self, index: usize, typed: &str) -> bool {
        let Some(&(at, slot)) = self.lines.get(index) else {
            return false;
        };

        let Some(field) = self.records.get(at).and_then(|record| record.fields.get(slot)) else {
            return false;
        };

        if let Some(held) = self.inputs.get_mut(index) {
            typed.clone_into(held);
        }

        let rendered = match &field.value {
            Value::Number(_) => match integer(typed) {
                Some(number) => number.to_string(),
                None => return false,
            },
            Value::Text(_) => json::quote(typed),
            Value::Numbers(_) => {
                let parsed: Option<Vec<i64>> =
                    typed.split(',').map(str::trim).filter(|item| !item.is_empty()).map(integer).collect();

                match parsed {
                    Some(items) => json::numbers(&self.text, &field.span, &items),
                    None => return false,
                }
            }
            Value::Texts(_) => {
                let items: Vec<String> = typed.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect();

                json::texts(&self.text, &field.span, &items)
            }
            Value::Tokens(_) => match json::tokenize(typed) {
                Some(items) => json::tokens(&self.text, &field.span, &items),
                None => return false,
            },
            Value::Flag(_) => match typed.trim() {
                "true" => "true".to_owned(),
                "false" => "false".to_owned(),
                _ => return false,
            },
            Value::Other => return false,
        };

        let kept = !matches!(field.value, Value::Number(_));
        let text = json::replace(&self.text, &field.span, &rendered);

        if !self.rewrite(text) {
            return false;
        }

        if kept && let Some(held) = self.inputs.get_mut(index) {
            typed.clone_into(held);
        }

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(vanilla) = fs::read_to_string(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Preset editor could not read the vanilla file");

            return false;
        };

        self.text = vanilla;
        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Preset editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Preset editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        match preview::save(&path, self.text.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Preset editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Choice {
    raw: i64,
    label: String,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Intent {
    Remove(u32),
    Sync,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Searched(String),
    Scrolled(f32),
    Line(usize, String),
    Typed(Spot, String),
    Form(u32, i64),
    Cannon(i64),
    Graded(&'static str, usize),
    Wiped(bool),
    Add,
    Remove(u32),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    query: String,
    shown: Vec<usize>,
    offset: f32,
    typed: Option<(Spot, String)>,
    confirm: Slot<Intent>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

fn list_id() -> Id {
    Id::new("preset-list")
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.query.clear();
        self.offset = 0.0;
        self.typed = None;
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
        self.refilter();
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::drifted)
    }

    fn refilter(&mut self) {
        let needle = self.query.trim().to_lowercase();

        self.shown = self.draft.as_ref().map_or_else(Vec::new, |draft| {
            (0..draft.labels.len())
                .filter(|index| {
                    needle.is_empty()
                        || draft.labels.get(*index).is_some_and(|label| label.to_lowercase().contains(&needle))
                        || draft.inputs.get(*index).is_some_and(|input| input.to_lowercase().contains(&needle))
                })
                .collect()
        });
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>, vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        let Some(plan) = plan else {
            self.draft = None;

            return;
        };

        let moved = current.plan.held(vfs).is_some() != (current.read_from != current.plan.game);

        if current.plan != plan || current.drifted() || moved {
            self.typed = None;
            self.draft = Draft::load(plan, vfs);
            self.refilter();
        }
    }

    fn settle(&mut self, changed: bool, vfs: &Vfs) -> Task<Message> {
        let Some(draft) = self.draft.as_mut() else {
            return Task::none();
        };

        if changed && !draft.persist(vfs) {
            return self.failed.set((), Message::FailureExpired);
        }

        Task::none()
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.draft = None;
                    self.typed = None;
                    self.confirm.clear();
                    self.failed.clear();
                }

                Task::none()
            }
            Message::Searched(query) => {
                self.query = query;
                self.offset = 0.0;
                self.refilter();

                iced::widget::operation::scroll_to(list_id(), scrollable::AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::Scrolled(offset) => {
                self.offset = offset;

                Task::none()
            }
            Message::Line(index, typed) => {
                let changed = self.draft.as_mut().is_some_and(|draft| draft.edit(index, &typed));

                self.settle(changed, vfs)
            }
            Message::Typed(spot, typed) => {
                if !digits(&typed) {
                    return Task::none();
                }

                let parsed = typed.parse::<i64>().ok();
                self.typed = Some((spot.clone(), typed));

                if spot == Spot::Adding {
                    return Task::none();
                }

                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                let Some(value) = parsed else {
                    return Task::none();
                };

                let (least, most) = draft.bounds(&spot);

                if value < least {
                    return Task::none();
                }

                let value = match most {
                    Some(most) if value > most => {
                        self.typed = None;

                        most
                    }
                    _ => value,
                };

                let changed = draft.set(&spot, value);

                self.settle(changed, vfs)
            }
            Message::Form(id, form) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.put_unit(id, EVOLUTION, form));

                self.settle(changed, vfs)
            }
            Message::Cannon(cannon) => {
                let changed = self.draft.as_mut().is_some_and(|draft| draft.set_cannon(cannon));

                self.settle(changed, vfs)
            }
            Message::Graded(chapter, grade) => {
                let changed = self.draft.as_mut().is_some_and(|draft| draft.grade(chapter, grade));

                self.settle(changed, vfs)
            }
            Message::Wiped(on) => {
                let changed = self.draft.as_mut().is_some_and(|draft| draft.wipe(on));

                self.settle(changed, vfs)
            }
            Message::Add => {
                let Some(id) = self.adding() else {
                    return Task::none();
                };

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.add(id));

                self.settle(changed, vfs)
            }
            Message::Remove(id) => {
                if !self.confirm.take(&Intent::Remove(id)) {
                    return self.confirm.set(Intent::Remove(id), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.drop_unit(id));

                self.settle(changed, vfs)
            }
            Message::Sync => {
                if !self.confirm.take(&Intent::Sync) {
                    return self.confirm.set(Intent::Sync, Message::ConfirmExpired);
                }

                self.typed = None;

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

                self.refilter();

                match restored {
                    true => Task::none(),
                    false => self.failed.set((), Message::FailureExpired),
                }
            }
            Message::ConfirmExpired => {
                self.confirm.expire();

                Task::none()
            }
            Message::FailureExpired => {
                self.failed.expire();

                Task::none()
            }
        }
    }

    fn adding(&self) -> Option<u32> {
        let draft = self.draft.as_ref()?;

        let id = match &self.typed {
            Some((Spot::Adding, typed)) => typed.parse::<u32>().ok()?,
            _ => return None,
        };

        let lineup = draft.lineup();

        (lineup.len() < LINEUP_SLOTS && !lineup.contains(&id) && draft.plan.member(id).is_some()).then_some(id)
    }

    fn typed_or(&self, spot: &Spot, current: Option<i64>) -> String {
        match &self.typed {
            Some((held, typed)) if held == spot => typed.clone(),
            _ => current.map_or_else(String::new, |value| value.to_string()),
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let content: Element<'_, Message> = match (draft.records.is_empty(), draft.plan.values) {
            (true, _) => framed(notice(UNREADABLE_LABEL)),
            (false, EditorMode::Raw) => self.raw(draft),
            (false, EditorMode::Resolved) => framed(self.resolved(draft)),
        };

        let label = match (self.failed.is_set() || draft.failed, self.confirm.armed_for(&Intent::Sync)) {
            (true, _) => FAILURE_LABEL,
            (false, true) => CONFIRM_LABEL,
            (false, false) => SYNC_LABEL,
        };

        let sync = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Sync);

        column![content, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let search = text_input(SEARCH_HINT, &self.query)
            .on_input(Message::Searched)
            .padding(INPUT_PADDING + 2.0)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let listing: Element<'_, Message> = match self.shown.is_empty() {
            true => notice(EMPTY_LABEL),
            false => responsive(move |size: Size| self.rows(draft, size)).into(),
        };

        column![search, framed(listing)].spacing(GAP).width(Length::Fill).height(Length::Fill).into()
    }

    fn rows<'a>(&'a self, draft: &'a Draft, size: Size) -> Element<'a, Message> {
        let RowWindow { range, pad_before, pad_after } =
            row_window::compute_with(self.shown.len(), size.height, self.offset, LINE_ROW, 0.0);

        let mut list = Column::with_capacity(range.len() + 2).width(Length::Fill);

        if pad_before > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_before)));
        }

        for at in range {
            let Some(&index) = self.shown.get(at) else {
                continue;
            };

            list = list.push(line(draft, index));
        }

        if pad_after > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_after)));
        }

        let area = scrollable(container(list).padding(Padding::default().right(SCROLL_GAP)))
            .id(list_id())
            .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
            .width(Length::Fill)
            .height(Length::Fill);

        smooth_scroll(area).into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let cards = column![self.lineup(draft), self.upgrades(draft), self.cannons(draft), self.treasures(draft)]
            .spacing(CARD_GAP)
            .width(Length::Fill);

        smooth_scroll(scrollable(container(cards).padding(Padding::default().right(SCROLL_GAP))).width(Length::Fill).height(Length::Fill))
            .into()
    }

    fn numeric<'a>(&'a self, draft: &'a Draft, spot: Spot) -> Element<'a, Message> {
        let shown = self.typed_or(&spot, draft.current(&spot));

        text_input("", &shown)
            .on_input(move |typed| Message::Typed(spot.clone(), typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fixed(NUMBER_WIDTH))
            .style(theme::rounded_input)
            .into()
    }

    fn lineup<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let plan = &draft.plan;

        let cannons: Vec<Choice> = (0..)
            .zip(plan.cannons.iter())
            .map(|(raw, label)| Choice { raw, label: label.clone() })
            .collect();

        let current = draft.slot_cannon();
        let chosen = cannons.iter().find(|choice| Some(choice.raw) == current).cloned();

        let cannon = pick_list(cannons, chosen, |picked: Choice| Message::Cannon(picked.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fixed(PICK_WIDTH))
            .style(theme::combo_box);

        let heading = row![text(LINEUP_HEADING).size(HEADING_SIZE).width(Length::Fill), cannon].align_y(Vertical::Center);

        let mut units = Column::new().spacing(UNIT_GAP).width(Length::Fill);
        let lineup = draft.lineup();

        for id in &lineup {
            units = units.push(self.member(draft, *id));
        }

        let typed = match &self.typed {
            Some((Spot::Adding, typed)) => typed.as_str(),
            _ => "",
        };

        let field = text_input(ADD_HINT, typed)
            .on_input(|typed| Message::Typed(Spot::Adding, typed))
            .on_submit(Message::Add)
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let preview = self.adding().map_or_else(String::new, |id| plan.name(id));

        let add = button(theme::centered_text(ADD_LABEL).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .style(theme::primary_button)
            .on_press_maybe(self.adding().map(|_| Message::Add));

        let adder = column![
            row![field, add].spacing(GAP).align_y(Vertical::Center),
            text(preview).size(NOTE_SIZE).style(text::secondary),
        ]
        .spacing(2);

        let mut body = column![heading, units].spacing(CARD_GAP);

        if lineup.len() < LINEUP_SLOTS {
            body = body.push(adder);
        }

        card(body.into())
    }

    fn member<'a>(&'a self, draft: &'a Draft, id: u32) -> Element<'a, Message> {
        let plan = &draft.plan;
        let armed = self.confirm.armed_for(&Intent::Remove(id));

        let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Remove(id));

        let named = row![text(plan.name(id)).size(LABEL_SIZE).width(Length::Fill), remove].align_y(Vertical::Center);

        let forms: Vec<Choice> = match plan.member(id) {
            Some(member) => (1..).zip(member.forms.iter()).map(|(raw, label)| Choice { raw, label: label.clone() }).collect(),
            None => (1..).zip(FORMS).map(|(raw, label)| Choice { raw, label: label.to_owned() }).collect(),
        };

        let unit = draft.unit(id);
        let form = unit.and_then(|record| record.field(EVOLUTION)).and_then(|field| number(&field.value));
        let chosen = forms.iter().find(|choice| Some(choice.raw) == form).cloned();

        let pick = pick_list(forms, chosen, move |picked: Choice| Message::Form(id, picked.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let levels = row![
            pick,
            text(LEVEL_MARK).size(LABEL_SIZE),
            self.numeric(draft, Spot::Level(id)),
            text(PLUS_MARK).size(LABEL_SIZE),
            self.numeric(draft, Spot::Plus(id)),
        ]
        .spacing(GAP)
        .align_y(Vertical::Center);

        let mut held = column![named, levels].spacing(4);

        if unit.is_none() {
            held = held.push(text(MISSING_LABEL).size(NOTE_SIZE).style(text::danger));
        }

        held.into()
    }

    fn upgrades<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut rows = column![text(UPGRADES_HEADING).size(HEADING_SIZE)].spacing(CARD_GAP);

        for (kind, key) in draft.keyed(&ABILITY) {
            let label = usize::try_from(kind).ok().and_then(|at| UPGRADE_NAMES.get(at)).map_or_else(|| key.clone(), |name| (*name).to_owned());

            rows = rows.push(
                row![
                    text(label).size(LABEL_SIZE).width(Length::Fill),
                    text(LEVEL_MARK).size(LABEL_SIZE),
                    self.numeric(draft, Spot::Upgrade(key.clone())),
                    text(PLUS_MARK).size(LABEL_SIZE),
                    self.numeric(draft, Spot::Boost(key)),
                ]
                .spacing(GAP)
                .align_y(Vertical::Center),
            );
        }

        card(rows.into())
    }

    fn cannons<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut rows = column![text(CANNONS_HEADING).size(HEADING_SIZE)].spacing(CARD_GAP);

        for (part, key) in draft.keyed(&CANNON) {
            rows = rows.push(
                row![
                    text(draft.plan.cannon(part)).size(LABEL_SIZE).width(Length::Fill),
                    text(LEVEL_MARK).size(LABEL_SIZE),
                    self.numeric(draft, Spot::Cannon(key)),
                ]
                .spacing(GAP)
                .align_y(Vertical::Center),
            );
        }

        card(rows.into())
    }

    fn treasures<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let wiped = draft.wiped();

        let wipe = toggler(wiped).label(WIPE_LABEL).text_size(LABEL_SIZE).on_toggle(Message::Wiped).style(theme::ios_toggle);

        let mut rows = column![text(TREASURES_HEADING).size(HEADING_SIZE), wipe].spacing(CARD_GAP);

        for (chapter, label) in CHAPTERS {
            let grades: Vec<Choice> = (0..).zip(GRADES).map(|(raw, label)| Choice { raw, label: label.to_owned() }).collect();
            let current = draft.grade_of(chapter);
            let chosen = current.and_then(|grade| grades.get(grade)).cloned();
            let hint = if wiped { GRADES[0] } else { SAVED_LABEL };

            let pick = pick_list(grades, chosen, move |picked: Choice| Message::Graded(chapter, usize::try_from(picked.raw).unwrap_or(0)))
                .placeholder(hint)
                .text_size(INPUT_SIZE)
                .width(Length::Fixed(PICK_WIDTH))
                .style(theme::combo_box);

            rows = rows.push(row![text(label).size(LABEL_SIZE).width(Length::Fill), pick].spacing(GAP).align_y(Vertical::Center));
        }

        card(rows.into())
    }
}

fn line(draft: &Draft, index: usize) -> Element<'_, Message> {
    let label = draft.labels.get(index).map_or("", String::as_str);
    let input = draft.inputs.get(index).map_or("", String::as_str);

    let editable = draft
        .lines
        .get(index)
        .and_then(|(at, slot)| draft.records.get(*at)?.fields.get(*slot))
        .is_some_and(|field| !matches!(field.value, Value::Other));

    let control: Element<'_, Message> = match editable {
        true => text_input("", input)
            .on_input(move |typed| Message::Line(index, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fixed(RAW_WIDTH))
            .style(theme::rounded_input)
            .into(),
        false => text(input).size(INPUT_SIZE).width(Length::Fixed(RAW_WIDTH)).into(),
    };

    let body = row![text(label).size(LABEL_SIZE).width(Length::Fill), control].spacing(GAP).align_y(Vertical::Center);

    container(body).height(Length::Fixed(LINE_ROW)).align_y(Vertical::Center).into()
}

fn card(body: Element<'_, Message>) -> Element<'_, Message> {
    container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
}

fn framed(body: Element<'_, Message>) -> Element<'_, Message> {
    container(body).width(Length::Fill).height(Length::Fill).padding(FRAME_INSET).style(theme::card_container_outlined).into()
}

fn notice(label: &str) -> Element<'_, Message> {
    container(theme::centered_text(label).size(LABEL_SIZE).style(text::danger))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    Mark::of(reader.text(&held), reader.text(&plan.game))
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{grade, Value};

    #[test]
    fn a_treasure_grade_is_its_highest_full_set() {
        let texts = |cells: [&str; 3]| Value::Texts(cells.iter().map(|cell| (*cell).to_owned()).collect());

        assert_eq!(grade(&texts(["0", "0", "48"])), 3);
        assert_eq!(grade(&texts(["48", "48", "0"])), 2, "the better grade wins, as the engine scans from the top");
        assert_eq!(grade(&texts(["0", "12", "0"])), 0, "a partial set counts for nothing");
        assert_eq!(grade(&Value::Numbers(vec!["48".into(), "0".into()])), 0, "two cells are too short to read");
    }
}
