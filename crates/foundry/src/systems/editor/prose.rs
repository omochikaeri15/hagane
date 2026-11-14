use super::{Mark, Reader, Realm};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use codex::combat::Separator;
use codex::common;
use tracing::warn;
use crate::common::preview::{self, Stamp};
use crate::Vfs;
use crate::domains::mods;
use crate::domains::stage::names;
use codex::combat::{Faction, Identity};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Subject {
    Explanation,
    EnemyName,
    EnemyDescription,
    ComboName,
    TalentText,
    MapName,
    StageName,
    ItemName,
    CatGlossary,
    EnemyGlossary,
}

impl Subject {
    pub fn slot(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Explanation => "Name & Description",
            Self::EnemyName => "Name",
            Self::EnemyDescription => "Description",
            Self::ComboName => "Combo Names",
            Self::TalentText => "Talent Text",
            Self::MapName => "Map Name",
            Self::StageName => "Stage Name",
            Self::ItemName => "Item Text",
            Self::CatGlossary | Self::EnemyGlossary => "Ability Glossary",
        }
    }

    pub fn labels(self) -> &'static [&'static str] {
        match self {
            Self::Explanation => EXPLANATION_LABELS,
            Self::EnemyName => ENEMY_NAME_LABELS,
            Self::EnemyDescription => ENEMY_DESCRIPTION_LABELS,
            Self::ComboName => COMBO_NAME_LABELS,
            Self::TalentText => TALENT_TEXT_LABELS,
            Self::MapName => MAP_NAME_LABELS,
            Self::StageName => STAGE_NAME_LABELS,
            Self::ItemName => ITEM_NAME_LABELS,
            Self::CatGlossary | Self::EnemyGlossary => GLOSSARY_LABELS,
        }
    }

    fn delimited(self) -> bool {
        !matches!(self, Self::EnemyName | Self::ComboName)
    }

    fn borrows(self) -> bool {
        matches!(self, Self::EnemyDescription | Self::TalentText)
    }

    pub fn pins(self) -> bool {
        matches!(self, Self::ComboName | Self::TalentText)
    }

    pub fn wrapped(self) -> Option<&'static str> {
        matches!(self, Self::TalentText).then_some(TALENT_BREAK)
    }

    pub fn notice(self) -> Option<&'static str> {
        match self {
            Self::TalentText => Some(TALENT_TEXT_NOTICE),
            Self::CatGlossary | Self::EnemyGlossary => Some(GLOSSARY_NOTICE),
            _ => None,
        }
    }

    pub fn glossary(self) -> bool {
        matches!(self, Self::CatGlossary | Self::EnemyGlossary)
    }

    fn skipped(self) -> usize {
        match self {
            Self::EnemyDescription | Self::TalentText | Self::MapName => 1,
            Self::Explanation
            | Self::EnemyName
            | Self::ComboName
            | Self::StageName
            | Self::ItemName
            | Self::CatGlossary
            | Self::EnemyGlossary => 0,
        }
    }
}

pub const COUNT: usize = 10;

pub const SUBJECTS: [Subject; COUNT] = [
    Subject::Explanation,
    Subject::EnemyName,
    Subject::EnemyDescription,
    Subject::ComboName,
    Subject::TalentText,
    Subject::MapName,
    Subject::StageName,
    Subject::ItemName,
    Subject::CatGlossary,
    Subject::EnemyGlossary,
];

const EXPLANATION_LABELS: &[&str] = &[
    "Name...",
    "Description Line 1...",
    "Description Line 2...",
    "Description Line 3...",
    "Comment...",
];

const ENEMY_NAME_LABELS: &[&str] = &["Name..."];

const COMBO_NAME_LABELS: &[&str] = &["Combo Name..."];

const MAP_NAME_LABELS: &[&str] = &["Map Name..."];

const ITEM_NAME_LABELS: &[&str] =
    &["Item Name...", "Description Line 1...", "Description Line 2...", "Description Line 3..."];

const STAGE_NAME_LABELS: &[&str] = &["Stage Name..."];

const GLOSSARY_LABELS: &[&str] = &["Name...", "Description Line 1...", "Description Line 2...", "Description Line 3..."];

const GLOSSARY_NOTICE: &str =
    "Every dictionary file is copied into the Mod together, so a moved line never drifts from its tables";

const TALENT_TEXT_LABELS: &[&str] = &["Description Line 1...", "Description Line 2..."];

pub const TALENT_BREAK: &str = "<br>";

const TALENT_TEXT_NOTICE: &str =
    "Descriptions are shared by every unit\nEditing one changes it for every talent that uses this text ID";

const ENEMY_DESCRIPTION_LABELS: &[&str] = &[
    "Description Line 1...",
    "Description Line 2...",
    "Description Line 3...",
    "Description Line 4...",
];

impl Subject {
    pub fn realm(self) -> Realm {
        match self {
            Self::Explanation | Self::ComboName | Self::TalentText | Self::CatGlossary => Realm::Cats,
            Self::EnemyName | Self::EnemyDescription | Self::EnemyGlossary => Realm::Enemies,
            Self::MapName | Self::StageName | Self::ItemName => Realm::Stages,
        }
    }
}

const GLOSSARY_TERMINATOR: &str = "＠";

const GLOSSARY_OPEN: char = '【';

const GLOSSARY_CLOSE: char = '】';

pub(super) const GLOSSARY_FILES: [&str; 4] = [
    "nyankoPictureBook2.csv",
    "EnemyPictureBook2.csv",
    "nyankoPictureBookData_EffectAbility.csv",
    "nyankoPictureBookData_Attribute.csv",
];

pub(super) static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

pub(super) fn next_token() -> u64 {
    NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Keyed {
    Map(u32),
    Stage(u32),
}

impl Keyed {
    pub fn locate(self, body: &str, delimiter: char) -> Option<usize> {
        match self {
            Self::Map(id) => names::map_name_rows(body, delimiter).get(&id).copied(),
            Self::Stage(id) => names::stage_name_rows(body, delimiter).get(&id).copied(),
        }
    }
}

#[derive(Clone)]
pub struct Plan {
    pub subject: Subject,
    pub row: usize,
    pub keyed: Option<Keyed>,
    pub rows: Vec<usize>,
    pub cell: Option<usize>,
    pub stated: Option<char>,
    pub label: String,
    pub file: String,
    pub game: PathBuf,
    pub target_mod: Option<String>,
    pub aim: Option<(Identity, Faction)>,
}

impl Plan {
    pub fn subject(&self) -> Subject {
        self.subject
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn over(self, rows: Vec<usize>) -> Plan {
        Plan { rows, ..self }
    }

    pub fn at(self, cell: usize) -> Plan {
        Plan { cell: Some(cell), ..self }
    }

    pub fn keyed(self, keyed: Keyed) -> Plan {
        Plan { keyed: Some(keyed), ..self }
    }

    pub(super) fn aiming(self, aim: Option<(Identity, Faction)>) -> Plan {
        Plan { aim, ..self }
    }

    pub fn aim(&self) -> Option<(Identity, Faction)> {
        self.aim
    }

    pub(super) fn sourced(self) -> Plan {
        let named = self.game.file_name().map(|name| name.to_string_lossy().into_owned());

        Plan { stated: named.as_deref().map(separator), ..self }
    }

    pub fn delimiter(&self) -> Option<char> {
        self.subject
            .delimited()
            .then(|| self.stated.unwrap_or_else(|| separator(&self.file)))
    }

    pub fn skip(&self) -> usize {
        self.cell.unwrap_or_else(|| self.subject.skipped())
    }

    pub fn matches(&self, other: &Plan) -> bool {
        self.row == other.row
            && self.keyed == other.keyed
            && self.cell == other.cell
            && self.stated == other.stated
            && self.game == other.game
            && self.target_mod == other.target_mod
            && self.label == other.label
    }

    pub fn source(&self, vfs: &Vfs) -> PathBuf {
        self.target_mod
            .as_deref()
            .and_then(|name| mods::find(vfs, name, Path::new(&self.file)))
            .unwrap_or_else(|| self.game.clone())
    }
}

pub struct Draft {
    pub plan: Plan,
    pub row: usize,
    pub read_from: PathBuf,
    pub stamp: Stamp,
    pub delimiter: Option<char>,
    pub lines: Vec<String>,
    pub head: Vec<String>,
    pub fields: Vec<String>,
    pub tail: Vec<String>,
    pub failed: bool,
    pub dirty: bool,
    pub writing: bool,
    pub token: u64,
}

pub fn entry(plan: &Plan, body: &str) -> Option<Vec<String>> {
    let delimiter = plan.delimiter();
    let row = seat(plan, body, delimiter)?;

    Some(parse(body, row, plan.skip(), delimiter, plan.subject).1)
}

fn stage_glossary(vfs: &Vfs, mod_name: &str) {
    for base in GLOSSARY_FILES {
        for name in super::copies(vfs, base) {
            let Some(source) = vfs.pristine(name.as_str()) else {
                continue;
            };

            if let Err(err) = mods::ensure_as(vfs, mod_name, &source, &name) {
                warn!(source = %source.display(), "Prose editor could not stage a dictionary file: {}", err);
            }
        }
    }
}

pub fn glossary_row(line: &str, delimiter: char) -> (Vec<String>, Vec<String>, Vec<String>) {
    let cells: Vec<&str> = line.split(delimiter).collect();
    let end = cells.iter().position(|cell| cell.trim().starts_with(GLOSSARY_TERMINATOR)).unwrap_or(cells.len());
    let (body, rest) = cells.split_at(end);
    let name = body
        .first()
        .map(|cell| cell.trim())
        .map(|cell| cell.strip_prefix(GLOSSARY_OPEN).and_then(|inner| inner.strip_suffix(GLOSSARY_CLOSE)).unwrap_or(cell))
        .unwrap_or_default();
    let mut fields = vec![name.to_owned()];

    fields.extend((1..GLOSSARY_LABELS.len()).map(|index| body.get(index).map_or(String::new(), |cell| cell.trim().to_owned())));

    (Vec::new(), fields, rest.iter().map(|cell| (*cell).to_owned()).collect())
}

pub fn glossary_join(fields: &[String], tail: &[String], delimiter: char) -> String {
    let name = fields.first().map(|name| name.trim()).unwrap_or_default();
    let mut cells: Vec<String> = Vec::new();

    if !name.is_empty() {
        cells.push(format!("{GLOSSARY_OPEN}{name}{GLOSSARY_CLOSE}"));
    }

    let mut lines: Vec<&str> = fields.iter().skip(1).map(|line| line.trim()).collect();

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    cells.extend(lines.into_iter().map(str::to_owned));
    cells.extend(tail.iter().cloned());

    cells.join(&delimiter.to_string())
}

pub fn write_now(path: &Path, body: &[u8], stamp: Stamp) -> Option<Stamp> {
    preview::save(path, body, stamp)
        .inspect_err(|err| warn!(path = %path.display(), "Prose editor could not write the file: {}", err))
        .ok()
}

pub const PIPE: char = '|';

#[derive(Clone, PartialEq, Eq)]
pub struct Aim {
    pub row: usize,
    pub label: String,
}

impl std::fmt::Display for Aim {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}  {}", self.row, fitted(&self.label))
    }
}

fn fitted(label: &str) -> String {
    const ROOM: usize = 48;

    if label.chars().count() <= ROOM {
        return label.to_owned();
    }

    label.chars().take(ROOM).chain("...".chars()).collect()
}

pub fn separator(name: &str) -> char {
    Separator::localized(name).char()
}

pub(super) fn seated(plan: &Plan) -> bool {
    let Some(keyed) = plan.keyed else {
        return true;
    };

    let Some(delimiter) = plan.delimiter() else {
        return true;
    };

    fs::read(&plan.game)
        .ok()
        .is_some_and(|bytes| keyed.locate(&common::scrub(&bytes), delimiter).is_some())
}

fn seat(plan: &Plan, body: &str, delimiter: Option<char>) -> Option<usize> {
    let Some(keyed) = plan.keyed else {
        return Some(plan.row);
    };

    keyed.locate(body, delimiter.unwrap_or(Separator::Comma.char()))
}

pub(super) fn parse(
    body: &str,
    index: usize,
    skip: usize,
    delimiter: Option<char>,
    subject: Subject,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let source = body.lines().nth(index).unwrap_or_default();

    if subject.glossary() {
        return glossary_row(source, delimiter.unwrap_or(PIPE));
    }

    let (head, fields, tail) = row(source, delimiter, skip, subject.labels().len(), subject.wrapped());

    if !subject.borrows() || filled(&head) {
        return (head, fields, tail);
    }

    let borrowed = body
        .lines()
        .map(|line| row(line, delimiter, skip, 0, None).0)
        .find(|candidate| filled(candidate));

    (borrowed.unwrap_or(head), fields, tail)
}

pub(super) fn filled(head: &[String]) -> bool {
    head.iter().any(|field| !field.trim().is_empty())
}

pub fn row(
    line: &str,
    delimiter: Option<char>,
    skip: usize,
    count: usize,
    wrapped: Option<&str>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let Some(delimiter) = delimiter else {
        return (Vec::new(), vec![line.trim().to_owned()], Vec::new());
    };

    let mut parts = line.split(delimiter);
    let head = (0..skip).map(|_| parts.next().unwrap_or_default().to_owned()).collect();

    let Some(marker) = wrapped else {
        let fields = (0..count).map(|_| parts.next().unwrap_or_default().trim().to_owned()).collect();

        return (head, fields, parts.map(str::to_owned).collect());
    };

    let held: Vec<&str> = parts.collect();
    let whole = held.join(&delimiter.to_string());
    let mut broken = whole.split(marker);
    let fields = (0..count).map(|_| broken.next().unwrap_or_default().trim().to_owned()).collect();

    (head, fields, Vec::new())
}

pub fn join(
    head: &[String],
    fields: &[String],
    tail: &[String],
    delimiter: Option<char>,
    wrapped: Option<&str>,
) -> String {
    let Some(delimiter) = delimiter else {
        return fields.first().cloned().unwrap_or_default();
    };

    let Some(marker) = wrapped else {
        let parts: Vec<&str> = head.iter().chain(fields).chain(tail).map(String::as_str).collect();

        return parts.join(&delimiter.to_string());
    };

    let mut written: Vec<&str> = fields.iter().map(String::as_str).collect();

    while written.last().is_some_and(|last| last.is_empty()) {
        written.pop();
    }

    let body = written.join(marker);
    let parts: Vec<&str> = head.iter().map(String::as_str).chain(std::iter::once(body.as_str())).collect();

    parts.join(&delimiter.to_string())
}

pub fn plan(
    subject: Subject,
    row: usize,
    label: String,
    file: String,
    game: &Path,
    target_mod: Option<String>,
) -> Plan {
    Plan {
        subject,
        row,
        keyed: None,
        rows: Vec::new(),
        cell: None,
        stated: None,
        label,
        file,
        game: game.to_path_buf(),
        target_mod,
        aim: None,
    }
}

impl Draft {
    pub fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Prose editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let body = common::scrub(&bytes);
        let delimiter = plan.delimiter();
        let lines: Vec<String> = body.lines().map(str::to_owned).collect();
        let row = seat(&plan, &body, delimiter)?;
        let (head, fields, tail) = parse(&body, row, plan.skip(), delimiter, plan.subject);

        Some(Draft {
            plan,
            row,
            read_from,
            stamp,
            delimiter,
            lines,
            head,
            fields,
            tail,
            failed: false,
            dirty: false,
            writing: false,
            token: next_token(),
        })
    }

    pub fn edit(&mut self, index: usize, value: String) {
        let Some(slot) = self.fields.get_mut(index) else {
            return;
        };

        if *slot == value {
            return;
        }

        *slot = value;
        self.stage();
    }

    pub fn restore(&mut self) {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Prose editor could not read the vanilla file");
            self.failed = true;

            return;
        };

        let body = common::scrub(&bytes);
        let delimiter = self.plan.delimiter();
        let Some(row) = seat(&self.plan, &body, delimiter) else {
            warn!(path = %self.plan.game.display(), "Prose editor could not find the row in the vanilla file");
            self.failed = true;

            return;
        };

        let (head, fields, tail) = parse(&body, row, self.plan.skip(), delimiter, self.plan.subject);
        self.head = head;
        self.fields = fields;
        self.tail = tail;
        self.stage();
    }

    pub fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        let Some(name) = self.plan.target_mod.as_deref() else {
            return Some((self.plan.game.clone(), self.stamp));
        };

        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, name, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Prose editor could not stage the file: {}", err))
            .ok()?;

        if self.plan.subject.glossary() {
            stage_glossary(vfs, name);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    pub fn stage(&mut self) {
        while self.lines.len() <= self.row {
            self.lines.push(String::new());
        }

        let joined = if self.plan.subject.glossary() {
            glossary_join(&self.fields, &self.tail, self.delimiter.unwrap_or(PIPE))
        } else {
            join(&self.head, &self.fields, &self.tail, self.delimiter, self.plan.subject.wrapped())
        };
        let Some(slot) = self.lines.get_mut(self.row) else {
            self.failed = true;

            return;
        };

        *slot = joined;
        self.dirty = true;
    }

    pub fn prepare_write(&mut self, vfs: &Vfs) -> Option<(PathBuf, Vec<u8>, Stamp, u64)> {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return None;
        };

        let mut body = self.lines.join("\n");
        body.push('\n');

        self.dirty = false;
        self.writing = true;

        Some((path, body.into_bytes(), stamp, self.token))
    }

    pub fn persist_now(&mut self, vfs: &Vfs) {
        if !self.dirty && !self.writing {
            return;
        }

        let Some((path, body, stamp, _)) = self.prepare_write(vfs) else {
            return;
        };

        match write_now(&path, &body, stamp) {
            Some(stamp) => {
                self.read_from = path;
                self.stamp = stamp;
                self.failed = false;
            }
            None => self.failed = true,
        }

        self.writing = false;
    }

    pub fn aim(&self, row: usize) -> Aim {
        let spoken = self
            .lines
            .get(row)
            .and_then(|line| line.split(self.delimiter.unwrap_or(PIPE)).nth(self.plan.subject.skipped()))
            .map(|text| text.replace(TALENT_BREAK, " "))
            .filter(|text| !text.trim().is_empty());

        Aim { row, label: spoken.unwrap_or_else(|| format!("Text {row}")) }
    }
}

pub fn reset(plan: Plan, vfs: &Vfs) -> bool {
    let Some(mut draft) = Draft::load(plan, vfs) else {
        return false;
    };

    draft.restore();

    if draft.failed {
        return false;
    }

    draft.persist_now(vfs);

    !draft.failed
}

pub fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.target_mod.as_deref().and_then(|name| mods::find(vfs, name, Path::new(&plan.file))) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| entry(plan, &body));
    let theirs = reader.text(&plan.game).and_then(|body| entry(plan, &body));

    Mark::of(ours, theirs)
}
