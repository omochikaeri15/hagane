use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::combat::Separator;
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
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Group, SIZE);

pub(super) const TABLE: &str = "Charagroup.csv";
pub(super) const TITLE: &str = "Unit Group";
pub(super) const FOLDER: &str = "Unit Groups";

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
const RAW_WIDTH: f32 = 200.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This group is no longer in the file";
const KIND_LABEL: &str = "Group Type";
const TEXT_LABEL: &str = "Description Text Key";
const UNITS_HEADING: &str = "Cats";
const EMPTY_UNITS: &str = "No cats in this group";
const ADD_LABEL: &str = "Add Cat";
const ADD_HINT: &str = "Cat ID";
const REMOVE_LABEL: &str = "Remove";
const UNKNOWN_CAT: &str = "Unknown Cat";
const UNKNOWN_KIND: &str = "Unknown Type";
const COLUMN_LABEL: &str = "Column";
const LIST_JOINT: &str = ", ";
const HEADER_MARK: &str = "//";

const KEY: usize = 0;
const TEXT: usize = 1;
const KIND: usize = 2;
const UNITS: usize = 3;

const KINDS: [(i64, &str); 3] = [(0, "Only These Cats"), (2, "Every Cat But These"), (3, "Cat Combo Members")];

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    key: u32,
    roster: Vec<(u32, String)>,
    described: Option<(String, String)>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) key: u32,
    pub(super) roster: Vec<(u32, String)>,
    pub(super) described: Option<(String, String)>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, key, mut roster, described } = seed;

    roster.sort_unstable_by_key(|(id, _)| *id);

    Plan { label, file, game, target_mod, values, key, roster, described }
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
        self.known(id).map_or_else(|| format!("{UNKNOWN_CAT} {id:03}"), |name| format!("{name} ({id:03})"))
    }
}

fn delimiter(body: &str) -> char {
    Separator::detect(body).unwrap_or(Separator::Comma).char()
}

fn keyed(line: &str, delimiter: char) -> Option<u32> {
    if line.trim_start().starts_with(HEADER_MARK) {
        return None;
    }

    line.split(delimiter).next()?.trim().parse().ok()
}

fn located(lines: &[String], key: u32, delimiter: char) -> Option<usize> {
    lines.iter().position(|line| keyed(line, delimiter) == Some(key))
}

fn entry(body: &str, key: u32) -> Option<String> {
    let delimiter = delimiter(body);

    body.lines().find(|line| keyed(line, delimiter) == Some(key)).map(str::to_owned)
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    delimiter: char,
    lines: Vec<String>,
    header: Vec<String>,
    row: Option<usize>,
    cells: Vec<String>,
    tail: String,
    trailing: bool,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Group editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let body = common::scrub(&bytes);
        let delimiter = delimiter(&body);
        let lines: Vec<String> = body.lines().map(str::to_owned).collect();

        let header = lines
            .first()
            .and_then(|line| line.trim_start().strip_prefix(HEADER_MARK))
            .map(|names| names.split(delimiter).map(|name| name.trim().to_owned()).collect())
            .unwrap_or_default();

        let mut draft = Draft {
            plan,
            read_from,
            stamp,
            delimiter,
            lines,
            header,
            row: None,
            cells: Vec::new(),
            tail: String::new(),
            trailing: false,
            failed: false,
        };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        self.row = located(&self.lines, self.plan.key, self.delimiter);

        let line = self.row.and_then(|at| self.lines.get(at)).map_or("", String::as_str);

        let (data, tail) = match line.find(HEADER_MARK) {
            Some(at) => line.split_at(at),
            None => (line, ""),
        };

        self.tail = tail.to_owned();
        self.cells = data.split(self.delimiter).map(str::to_owned).collect();
        self.trailing = self.cells.len() > UNITS && self.cells.last().is_some_and(|cell| cell.trim().is_empty());

        if self.trailing {
            self.cells.pop();
        }

        while self.cells.len() < UNITS {
            self.cells.push(String::new());
        }
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn cell(&self, column: usize) -> &str {
        self.cells.get(column).map_or("", |cell| cell.trim())
    }

    fn kind(&self) -> Option<i64> {
        self.cell(KIND).parse().ok()
    }

    fn units(&self) -> Vec<u32> {
        self.cells.iter().skip(UNITS).filter_map(|cell| cell.trim().parse().ok()).collect()
    }

    fn stage(&mut self) -> bool {
        let Some(at) = self.row else {
            return false;
        };

        let mut line = self.cells.join(&self.delimiter.to_string());

        if self.trailing {
            line.push(self.delimiter);
        }

        line.push_str(&self.tail);

        let Some(slot) = self.lines.get_mut(at) else {
            return false;
        };

        if *slot == line {
            return false;
        }

        *slot = line;

        true
    }

    fn write(&mut self, column: usize, value: &str) -> bool {
        if self.row.is_none() || column >= UNITS {
            return false;
        }

        if let Some(cell) = self.cells.get_mut(column) {
            value.clone_into(cell);
        }

        self.stage()
    }

    fn set_units(&mut self, units: &[u32]) -> bool {
        if self.row.is_none() {
            return false;
        }

        self.cells.truncate(UNITS);
        self.cells.extend(units.iter().map(u32::to_string));

        self.stage()
    }

    fn set_listed(&mut self, typed: &str) -> bool {
        let parsed: Option<Vec<u32>> = typed.split(',').map(str::trim).filter(|cell| !cell.is_empty()).map(|cell| cell.parse().ok()).collect();

        parsed.is_some_and(|units| self.set_units(&units))
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Group editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.key) else {
            warn!(path = %self.plan.game.display(), "Group editor found no vanilla copy of this group");

            return false;
        };

        let Some(slot) = self.row.and_then(|at| self.lines.get_mut(at)) else {
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
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Group editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Group editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Group editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Choice {
    raw: i64,
    label: &'static str,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label)
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Intent {
    Remove(usize),
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Text,
    Kind,
    Units,
    Adding,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(Field, String),
    Kind(i64),
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
                    Field::Text => draft.write(TEXT, &typed),
                    Field::Kind => {
                        let body = typed.strip_prefix('-').unwrap_or(&typed);

                        match !body.is_empty() && body.chars().all(|glyph| glyph.is_ascii_digit()) {
                            true => draft.write(KIND, &typed),
                            false => false,
                        }
                    }
                    Field::Units => draft.set_listed(&typed),
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
            Message::Kind(raw) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(KIND, &raw.to_string()));

                self.settle(changed, vfs)
            }
            Message::Add => {
                let Some(id) = self.adding() else {
                    return Task::none();
                };

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut units = draft.units();

                    units.push(id);

                    draft.set_units(&units)
                });

                self.settle(changed, vfs)
            }
            Message::Remove(at) => {
                if !self.confirm.take(&Intent::Remove(at)) {
                    return self.confirm.set(Intent::Remove(at), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut units = draft.units();

                    if at < units.len() {
                        units.remove(at);
                    }

                    draft.set_units(&units)
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

        (!draft.units().contains(&id) && draft.plan.known(id).is_some()).then_some(id)
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
        let listing: Element<'_, Message> = match (draft.row.is_some(), draft.plan.values) {
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

    fn heading(draft: &Draft, column: usize) -> String {
        draft.header.get(column).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| format!("{COLUMN_LABEL} {}", column + 1))
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let input = |field: Field, current: String| {
            text_input("", &self.typed_or(field, current))
                .on_input(move |typed| Message::Typed(field, typed))
                .padding(INPUT_PADDING)
                .size(INPUT_SIZE)
                .width(Length::Fixed(RAW_WIDTH))
                .style(theme::rounded_input)
        };

        let line = |label: String, control: Element<'a, Message>| -> Element<'a, Message> {
            row![text(label).size(LABEL_SIZE).width(Length::Fill), control].spacing(GAP).align_y(Vertical::Center).into()
        };

        let listed: Vec<String> = draft.units().iter().map(u32::to_string).collect();

        column![
            line(Self::heading(draft, KEY), text(draft.cell(KEY).to_owned()).size(INPUT_SIZE).width(Length::Fixed(RAW_WIDTH)).into()),
            line(Self::heading(draft, TEXT), input(Field::Text, draft.cell(TEXT).to_owned()).into()),
            line(Self::heading(draft, KIND), input(Field::Kind, draft.cell(KIND).to_owned()).into()),
            line(Self::heading(draft, UNITS), input(Field::Units, listed.join(LIST_JOINT)).into()),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let plan = &draft.plan;
        let options: Vec<Choice> = KINDS.iter().map(|(raw, label)| Choice { raw: *raw, label }).collect();
        let current = draft.kind();
        let chosen = options.iter().find(|choice| Some(choice.raw) == current).cloned();

        let kind = pick_list(options, chosen, |picked: Choice| Message::Kind(picked.raw))
            .placeholder(UNKNOWN_KIND)
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let key = draft.cell(TEXT).to_owned();

        let key_input = text_input("", &self.typed_or(Field::Text, key.clone()))
            .on_input(|typed| Message::Typed(Field::Text, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let described = plan.described.as_ref().filter(|(held, _)| *held == key).map_or_else(String::new, |(_, words)| words.clone());

        let settings = card(
            column![
                column![text(KIND_LABEL).size(LABEL_SIZE), kind].spacing(2),
                column![text(TEXT_LABEL).size(LABEL_SIZE), key_input, text(described).size(NOTE_SIZE).style(text::secondary)].spacing(2),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        let mut units = Column::new().spacing(UNIT_GAP).width(Length::Fill);
        let held = draft.units();

        for (at, id) in held.iter().enumerate() {
            let armed = self.confirm.armed_for(&Intent::Remove(at));

            let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                .padding(SMALL_PADDING)
                .style(theme::danger_button)
                .on_press(Message::Remove(at));

            units = units.push(row![text(plan.name(*id)).size(LABEL_SIZE).width(Length::Fill), remove].spacing(GAP).align_y(Vertical::Center));
        }

        if held.is_empty() {
            units = units.push(text(EMPTY_UNITS).size(NOTE_SIZE).style(text::secondary));
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

        let members = card(
            column![
                text(UNITS_HEADING).size(HEADING_SIZE),
                units,
                row![field, add].spacing(GAP).align_y(Vertical::Center),
                text(preview).size(NOTE_SIZE).style(text::secondary),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        column![settings, members].spacing(CARD_GAP).width(Length::Fill).into()
    }
}

fn card(body: Element<'_, Message>) -> Element<'_, Message> {
    container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| entry(&body, plan.key));
    let theirs = reader.text(&plan.game).and_then(|body| entry(&body, plan.key));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{entry, keyed};

    #[test]
    fn a_group_is_found_by_its_leading_id_and_never_by_the_header() {
        let body = "//group_id,text_id,group_type,chara_id\n1,key_1,0,249,250\n10,key_10,3,831\n";

        assert_eq!(keyed("//1,a", ','), None);
        assert_eq!(entry(body, 10).as_deref(), Some("10,key_10,3,831"));
        assert_eq!(entry(body, 2), None);
    }
}
