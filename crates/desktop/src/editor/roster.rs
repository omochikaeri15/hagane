use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 480.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Roster, SIZE);

pub(super) const FOLDER: &str = "Gacha";
pub(super) const SET_NAME: &str = "Set";

pub(super) const CAPSULES: [(&str, &str); 3] = [("R", "Rare Capsules"), ("N", "Normal Capsules"), ("E", "Event Capsules")];
pub(super) const TIERS: [Tier; 3] = [Tier::Units, Tier::Second, Tier::Third];

pub(super) const PREFIX: &str = "GatyaDataSet";
const EXTENSION: &str = "csv";
const OPTIONS: &str = "GatyaData_Option_Set";
const OPTIONS_EXTENSION: &str = "tsv";

pub(super) const CHANCE_TABLE: &str = "GatyaData_Option_ChanceAnimation.tsv";
pub(super) const CHANCE_NAME: &str = "Rule";
pub(super) const CHANCE_FOLDER: &str = "Chance Animations";
const ANY_SERIES: &str = "Any Series";
const SERIES_LABEL: &str = "Series";

pub(super) fn chance_name(capsule: Option<i64>, series: Option<i64>) -> Option<String> {
    let heading = CAPSULES.get(usize::try_from(capsule?).ok()?).map(|(_, heading)| *heading)?;

    let series = match series? {
        -1 => ANY_SERIES.to_owned(),
        held => format!("{SERIES_LABEL} {held}"),
    };

    Some(format!("{heading}, {series}"))
}

pub(super) fn options_file(capsule: &str) -> String {
    format!("{OPTIONS}{capsule}.{OPTIONS_EXTENSION}")
}

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const UNIT_GAP: f32 = 4.0;
const HEADING_SIZE: f32 = 14.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This set is no longer in the file";
const RAW_LABEL: &str = "Entries";
const RAW_NOTE: &str = "Comma-separated, in the order the file holds them\nThe closing -1 is kept for you";
const UNITS_NOTE: &str = "The cats this capsule set can roll";
const ENTRIES_NOTE: &str = "The game reads this list for the set, but nothing it rolls is known to come from it";
const EMPTY_UNITS: &str = "Nothing in this list";
const ADD_LABEL: &str = "Add";
const ADD_HINT: &str = "ID";
const REMOVE_LABEL: &str = "Remove";
const UNKNOWN_CAT: &str = "Unknown Cat";
const ENTRY_LABEL: &str = "Entry";
const LIST_JOINT: &str = ", ";
const DELIMITER: char = ',';
const END: &str = "-1";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Tier {
    Units,
    Second,
    Third,
}

impl Tier {
    pub(super) fn title(self) -> &'static str {
        match self {
            Tier::Units => "Unit Roster",
            Tier::Second => "Second Roster",
            Tier::Third => "Third Roster",
        }
    }

    fn number(self) -> usize {
        match self {
            Tier::Units => 1,
            Tier::Second => 2,
            Tier::Third => 3,
        }
    }

    pub(super) fn file(self, capsule: &str) -> String {
        format!("{PREFIX}{capsule}{}.{EXTENSION}", self.number())
    }

    fn unique(self) -> bool {
        self == Tier::Units
    }

    fn note(self) -> &'static str {
        match self {
            Tier::Units => UNITS_NOTE,
            Tier::Second | Tier::Third => ENTRIES_NOTE,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    tier: Tier,
    line: usize,
    roster: Vec<(u32, String)>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) tier: Tier,
    pub(super) line: usize,
    pub(super) roster: Vec<(u32, String)>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, tier, line, mut roster } = seed;

    if !tier.unique() {
        roster.clear();
    }

    roster.sort_unstable_by_key(|(id, _)| *id);

    Plan { label, file, game, target_mod, values, tier, line, roster }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn known(&self, id: u32) -> Option<&str> {
        self.roster.binary_search_by_key(&id, |(held, _)| *held).ok().and_then(|at| self.roster.get(at)).map(|(_, name)| name.as_str())
    }

    fn name(&self, id: u32) -> String {
        match self.tier.unique() {
            true => self.known(id).map_or_else(|| format!("{UNKNOWN_CAT} {id:03}"), |name| format!("{name} ({id:03})")),
            false => format!("{ENTRY_LABEL} {id}"),
        }
    }
}

fn split(line: &str) -> (Vec<String>, String) {
    let cells: Vec<&str> = line.split(DELIMITER).collect();
    let end = cells.iter().position(|cell| cell.trim() == END).unwrap_or(cells.len());
    let head = cells.get(..end).unwrap_or_default().iter().filter(|cell| !cell.trim().is_empty()).map(|cell| (*cell).to_owned()).collect();
    let tail = cells.get(end..).map_or_else(String::new, |rest| rest.join(&DELIMITER.to_string()));

    (head, tail)
}

fn joined(entries: &[String], tail: &str) -> String {
    let tail = match tail.is_empty() {
        true => END,
        false => tail,
    };

    entries.iter().map(String::as_str).chain([tail]).collect::<Vec<&str>>().join(&DELIMITER.to_string())
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    cells: Vec<String>,
    tail: String,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Roster editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let mut draft = Draft { plan, read_from, stamp, lines, cells: Vec::new(), tail: String::new(), failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let (cells, tail) = self.lines.get(self.plan.line).map_or_else(|| (Vec::new(), String::new()), |line| split(line));

        self.cells = cells;
        self.tail = tail;
    }

    fn present(&self) -> bool {
        self.plan.line < self.lines.len()
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn entries(&self) -> Vec<u32> {
        self.cells.iter().filter_map(|cell| cell.trim().parse().ok()).collect()
    }

    fn set_entries(&mut self, entries: &[u32]) -> bool {
        let cells: Vec<String> = entries.iter().map(u32::to_string).collect();

        self.stage(cells)
    }

    fn stage(&mut self, cells: Vec<String>) -> bool {
        let line = joined(&cells, &self.tail);

        let Some(slot) = self.lines.get_mut(self.plan.line) else {
            return false;
        };

        if *slot == line {
            return false;
        }

        *slot = line;
        self.cells = cells;

        if self.tail.is_empty() {
            END.clone_into(&mut self.tail);
        }

        true
    }

    fn set_listed(&mut self, typed: &str) -> bool {
        let parsed: Option<Vec<u32>> = typed.split(DELIMITER).map(str::trim).filter(|cell| !cell.is_empty()).map(|cell| cell.parse().ok()).collect();

        parsed.is_some_and(|entries| self.set_entries(&entries))
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Roster editor could not read the vanilla file");

            return false;
        };

        let Some(original) = common::scrub(&bytes).lines().nth(self.plan.line).map(str::to_owned) else {
            warn!(path = %self.plan.game.display(), "Roster editor found no vanilla copy of this set");

            return false;
        };

        let Some(slot) = self.lines.get_mut(self.plan.line) else {
            return false;
        };

        *slot = original;
        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Roster editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Roster editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        let mut body = self.lines.join("\n");
        body.push('\n');

        match preview::save(&path, body.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Roster editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Intent {
    Remove(usize),
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Listed,
    Adding,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(Field, String),
    Add,
    Remove(usize),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<(Field, String)>,
    confirm: Slot<Intent>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.typed = None;
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
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
            Message::Typed(field, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                let changed = match field {
                    Field::Listed => draft.set_listed(&typed),
                    Field::Adding => {
                        if !typed.chars().all(|glyph| glyph.is_ascii_digit()) {
                            return Task::none();
                        }

                        false
                    }
                };

                self.typed = Some((field, typed));

                self.settle(changed, vfs)
            }
            Message::Add => {
                let Some(id) = self.adding() else {
                    return Task::none();
                };

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut entries = draft.entries();

                    entries.push(id);

                    draft.set_entries(&entries)
                });

                self.settle(changed, vfs)
            }
            Message::Remove(at) => {
                if !self.confirm.take(&Intent::Remove(at)) {
                    return self.confirm.set(Intent::Remove(at), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut entries = draft.entries();

                    if at < entries.len() {
                        entries.remove(at);
                    }

                    draft.set_entries(&entries)
                });

                self.settle(changed, vfs)
            }
            Message::Sync => {
                if !self.confirm.take(&Intent::Sync) {
                    return self.confirm.set(Intent::Sync, Message::ConfirmExpired);
                }

                self.typed = None;

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

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
            Some((Field::Adding, typed)) => typed.parse::<u32>().ok()?,
            _ => return None,
        };

        let plan = &draft.plan;

        match plan.tier.unique() {
            true => (!draft.entries().contains(&id) && plan.known(id).is_some()).then_some(id),
            false => Some(id),
        }
    }

    fn typed_or(&self, field: Field, current: String) -> String {
        match &self.typed {
            Some((held, typed)) if *held == field => typed.clone(),
            _ => current,
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listing: Element<'_, Message> = match (draft.present(), draft.plan.values) {
            (false, _) => container(theme::centered_text(MISSING_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            (true, EditorMode::Raw) => self.raw(draft),
            (true, EditorMode::Resolved) => self.resolved(draft),
        };

        let framed = container(
            smooth_scroll(scrollable(container(listing).padding(Padding::default().right(SCROLL_GAP))).width(Length::Fill).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(FRAME_INSET)
        .style(theme::card_container_outlined);

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

        column![framed, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listed: Vec<String> = draft.entries().iter().map(u32::to_string).collect();

        let input = text_input("", &self.typed_or(Field::Listed, listed.join(LIST_JOINT)))
            .on_input(|typed| Message::Typed(Field::Listed, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        column![text(RAW_LABEL).size(LABEL_SIZE), input, text(RAW_NOTE).size(NOTE_SIZE).style(text::secondary)].spacing(CARD_GAP).into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let plan = &draft.plan;
        let mut entries = Column::new().spacing(UNIT_GAP).width(Length::Fill);
        let held = draft.entries();

        for (at, id) in held.iter().enumerate() {
            let armed = self.confirm.armed_for(&Intent::Remove(at));

            let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                .padding(SMALL_PADDING)
                .style(theme::danger_button)
                .on_press(Message::Remove(at));

            entries = entries.push(row![text(plan.name(*id)).size(LABEL_SIZE).width(Length::Fill), remove].spacing(GAP).align_y(Vertical::Center));
        }

        if held.is_empty() {
            entries = entries.push(text(EMPTY_UNITS).size(NOTE_SIZE).style(text::secondary));
        }

        let typed = match &self.typed {
            Some((Field::Adding, typed)) => typed.as_str(),
            _ => "",
        };

        let field = text_input(ADD_HINT, typed)
            .on_input(|typed| Message::Typed(Field::Adding, typed))
            .on_submit(Message::Add)
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let add = button(theme::centered_text(ADD_LABEL).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .style(theme::primary_button)
            .on_press_maybe(self.adding().map(|_| Message::Add));

        let preview = self.adding().map_or_else(String::new, |id| plan.name(id));

        container(
            column![
                text(plan.tier.title()).size(HEADING_SIZE),
                text(plan.tier.note()).size(NOTE_SIZE).style(text::secondary),
                entries,
                row![field, add].spacing(GAP).align_y(Vertical::Center),
                text(preview).size(NOTE_SIZE).style(text::secondary),
            ]
            .spacing(CARD_GAP),
        )
        .width(Length::Fill)
        .padding(CARD_PADDING)
        .style(theme::card_container_outlined)
        .into()
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| body.lines().nth(plan.line).map(str::to_owned));
    let theirs = reader.text(&plan.game).and_then(|body| body.lines().nth(plan.line).map(str::to_owned));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{joined, split};

    #[test]
    fn a_roster_keeps_its_closing_mark_through_an_edit() {
        let (cells, tail) = split("153,152,30,-1");

        assert_eq!(cells, ["153", "152", "30"]);
        assert_eq!(joined(&["7".to_owned()], &tail), "7,-1");
        assert_eq!(joined(&[], &tail), "-1");
    }

    #[test]
    fn an_empty_roster_reads_as_nothing_and_never_loses_its_end() {
        assert!(split("-1").0.is_empty());
        assert_eq!(split("0,0,3,-1").0, ["0", "0", "3"]);
        assert_eq!(joined(&["4".to_owned()], ""), "4,-1");
    }
}
