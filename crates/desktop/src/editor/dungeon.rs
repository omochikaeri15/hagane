use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Row};
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

const SIZE: Size = Size::new(460.0, 560.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Dungeon, SIZE);

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
const NUMBER_WIDTH: f32 = 70.0;
const PIECE_WIDTH: f32 = 40.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const EMPTY_LABEL: &str = "This file has no layout rows";
const KINDS_HEADING: &str = "Stages Per Piece Kind";
const KIND_LABEL: &str = "Kind";
const TEMPLATE_HEADING: &str = "Template Order";
const MAP_HEADING: &str = "Map Layout";
const ID_LABEL: &str = "Map Layout ID";
const SECTION_LABEL: &str = "Section";
const STAGES_LABEL: &str = "Stages";
const SCATTER_LABELS: [&str; 3] = ["Scatter From Point", "Scatter To Point", "Extra Height"];
const ADD_KIND_LABEL: &str = "Add Kind";
const ADD_PIECE_LABEL: &str = "Add Piece";
const REMOVE_LABEL: &str = "Remove Last";
const RAW_FIRST_HEADING: &str = "Row 1";
const RAW_SECOND_HEADING: &str = "Row 2";
const RAW_ID_LABEL: &str = "ID";
const RAW_ENTRY_LABEL: &str = "Entry";
const RAW_VALUE_LABEL: &str = "Value";
const RAW_ADD_LABEL: &str = "Add Cell";
const KINDS_NOTE: &str = "A map is stacked from pieces\nEach kind of piece holds this many stages";
const TEMPLATE_NOTE: &str = "The kinds in the order the map's own layout file draws them\nThe game uses this to measure where each kind's stages sit, and the gap between two kinds next to each other\nEvery kind (and every pair of neighbors) a section uses must show up here";
const NOTE: &str = "A section is the stack of pieces that one stage set of the map is built from\nIts stage count is the total of what its pieces hold\nThe tower map scatters its drifting background objects between the two scatter points (counted from 0), plus the extra height\nOther maps ignore those three values";
const RAW_NOTE: &str = "After the first two rows, a row with a single cell starts a map\nEvery entry under it is a row of pieces followed by a row of three values";
const DELIMITER: char = ',';
const DEFAULT_CELL: &str = "0";
const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";

const KINDS_LINE: usize = 0;
const TEMPLATE_LINE: usize = 1;
const FIRST_MAP: usize = 2;
const SCATTER_CELLS: usize = 3;
const EXTRA_CELL: usize = 2;

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
}

pub(super) fn plan(label: String, file: String, game: PathBuf, target_mod: String, values: EditorMode) -> Plan {
    Plan { label, file, game, target_mod, values }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Stack {
    pieces: usize,
    scatter: usize,
}

#[derive(Clone, PartialEq, Debug)]
struct Map {
    head: usize,
    stacks: Vec<Stack>,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Count,
    Kind,
    Id,
    Scatter,
}

fn cell(line: &str, at: usize) -> &str {
    line.split(DELIMITER).nth(at).map_or("", str::trim)
}

fn number(line: &str, at: usize) -> Option<i64> {
    cell(line, at).parse().ok()
}

fn width(line: &str) -> usize {
    line.split(DELIMITER).count()
}

fn replaced(line: &str, at: usize, value: &str) -> String {
    let mut cells: Vec<&str> = line.split(DELIMITER).collect();

    while cells.len() <= at {
        cells.push("");
    }

    if let Some(slot) = cells.get_mut(at) {
        *slot = value;
    }

    cells.join(&DELIMITER.to_string())
}

fn continues(line: &str) -> bool {
    number(line, 0).is_some() && width(line) != 1 && number(line, 1).is_some()
}

fn maps(lines: &[String]) -> Vec<Map> {
    let mut found = Vec::new();
    let mut at = FIRST_MAP;

    while lines.get(at).is_some_and(|line| number(line, 0).is_some()) {
        let head = at;
        let mut stacks = Vec::new();

        at += 1;

        loop {
            stacks.push(Stack { pieces: at, scatter: at + 1 });
            at += 2;

            if !lines.get(at).is_some_and(|line| continues(line)) {
                break;
            }
        }

        found.push(Map { head, stacks });
    }

    found
}

fn letter(at: usize) -> String {
    LETTERS.get(at).map_or_else(|| (at + 1).to_string(), |glyph| char::from(*glyph).to_string())
}

fn numeric(typed: &str) -> bool {
    let digits = typed.strip_prefix('-').unwrap_or(typed);

    !digits.is_empty() && digits.chars().all(|glyph| glyph.is_ascii_digit())
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    maps: Vec<Map>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Floor layout editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let maps = maps(&lines);

        Some(Draft { plan, read_from, stamp, lines, maps, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn line(&self, at: usize) -> &str {
        self.lines.get(at).map_or("", String::as_str)
    }

    fn kinds(&self) -> usize {
        self.lines.get(KINDS_LINE).map_or(0, |line| width(line))
    }

    fn stages(&self, stack: &Stack) -> i64 {
        let counts = self.line(KINDS_LINE);

        self.line(stack.pieces)
            .split(DELIMITER)
            .filter_map(|kind| kind.trim().parse::<usize>().ok())
            .filter_map(|kind| number(counts, kind))
            .sum()
    }

    fn role(&self, line: usize) -> Role {
        if line == KINDS_LINE {
            return Role::Count;
        }

        if line == TEMPLATE_LINE {
            return Role::Kind;
        }

        if self.maps.iter().any(|map| map.head == line) {
            return Role::Id;
        }

        match self.maps.iter().flat_map(|map| map.stacks.iter()).any(|stack| stack.scatter == line) {
            true => Role::Scatter,
            false => Role::Kind,
        }
    }

    fn removable(&self, line: usize) -> bool {
        let follower = self.maps.iter().any(|map| map.stacks.iter().skip(1).any(|stack| stack.pieces == line));

        width(self.line(line)) > 1 + usize::from(follower)
    }

    fn fits(&self, line: usize, at: usize, typed: &str) -> bool {
        let Ok(value) = typed.parse::<i64>() else {
            return false;
        };

        if self.plan.values == EditorMode::Raw {
            return true;
        }

        match self.role(line) {
            Role::Count | Role::Id => value >= 0,
            Role::Kind => usize::try_from(value).is_ok_and(|kind| kind < self.kinds()),
            Role::Scatter => at == EXTRA_CELL || value >= 0,
        }
    }

    fn write(&mut self, line: usize, at: usize, value: &str) -> bool {
        while self.lines.len() <= line {
            self.lines.push(String::new());
        }

        let Some(slot) = self.lines.get_mut(line) else {
            return false;
        };

        let fresh = match slot.is_empty() {
            true => (0..SCATTER_CELLS).map(|held| if held == at { value } else { DEFAULT_CELL }).collect::<Vec<&str>>().join(&DELIMITER.to_string()),
            false => replaced(slot, at, value),
        };

        if *slot == fresh {
            return false;
        }

        *slot = fresh;
        self.maps = maps(&self.lines);

        true
    }

    fn add(&mut self, line: usize) -> bool {
        let Some(slot) = self.lines.get_mut(line) else {
            return false;
        };

        let last = slot.rsplit(DELIMITER).next().map_or(DEFAULT_CELL, str::trim).to_owned();
        let last = if numeric(&last) { last } else { DEFAULT_CELL.to_owned() };

        slot.push(DELIMITER);
        slot.push_str(&last);
        self.maps = maps(&self.lines);

        true
    }

    fn remove(&mut self, line: usize) -> bool {
        if !self.removable(line) {
            return false;
        }

        let Some(slot) = self.lines.get_mut(line) else {
            return false;
        };

        let Some((kept, _)) = slot.rsplit_once(DELIMITER) else {
            return false;
        };

        *slot = kept.to_owned();
        self.maps = maps(&self.lines);

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Floor layout editor could not read the vanilla file");

            return false;
        };

        self.lines = common::scrub(&bytes).lines().map(str::to_owned).collect();
        self.maps = maps(&self.lines);

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Floor layout editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Floor layout editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Floor layout editor could not write the file: {}", err);
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

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(usize, usize, String),
    Add(usize),
    Remove(usize),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<(usize, usize, String)>,
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
            Message::Typed(line, at, typed) => {
                let changed = numeric(&typed) && self.draft.as_mut().is_some_and(|draft| draft.fits(line, at, &typed) && draft.write(line, at, &typed));

                self.typed = Some((line, at, typed));

                self.settle(changed, vfs)
            }
            Message::Add(line) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.add(line));

                self.settle(changed, vfs)
            }
            Message::Remove(line) => {
                if !self.confirm.take(&Intent::Remove(line)) {
                    return self.confirm.set(Intent::Remove(line), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.remove(line));

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

    fn typed_or(&self, line: usize, at: usize, current: &str) -> String {
        match &self.typed {
            Some((held_line, held_at, typed)) if *held_line == line && *held_at == at => typed.clone(),
            _ => current.to_owned(),
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listing: Element<'_, Message> = match draft.lines.is_empty() {
            true => container(theme::centered_text(EMPTY_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => self.listing(draft),
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

    fn number<'a>(&'a self, draft: &'a Draft, line: usize, at: usize, width: f32) -> Element<'a, Message> {
        text_input("", &self.typed_or(line, at, cell(draft.line(line), at)))
            .on_input(move |typed| Message::Typed(line, at, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fixed(width))
            .style(theme::rounded_input)
            .into()
    }

    fn resize<'a>(&'a self, draft: &'a Draft, line: usize, add: &'a str) -> Element<'a, Message> {
        let armed = self.confirm.armed_for(&Intent::Remove(line));

        let grow = button(theme::centered_text(add).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .style(theme::primary_button)
            .on_press(Message::Add(line));

        let shrink = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .style(theme::danger_button)
            .on_press_maybe(draft.removable(line).then_some(Message::Remove(line)));

        row![grow, shrink].spacing(GAP).align_y(Vertical::Center).into()
    }

    fn strip<'a>(&'a self, draft: &'a Draft, line: usize) -> Element<'a, Message> {
        let mut cells = Row::new().spacing(UNIT_GAP);

        for at in 0..width(draft.line(line)) {
            cells = cells.push(self.number(draft, line, at, PIECE_WIDTH));
        }

        cells.wrap().vertical_spacing(UNIT_GAP).into()
    }

    fn listing<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let raw = draft.plan.values == EditorMode::Raw;
        let mut cards = Column::new().spacing(CARD_GAP).width(Length::Fill);

        let (kinds_heading, template_heading, add_kind, add_piece) = match raw {
            true => (RAW_FIRST_HEADING, RAW_SECOND_HEADING, RAW_ADD_LABEL, RAW_ADD_LABEL),
            false => (KINDS_HEADING, TEMPLATE_HEADING, ADD_KIND_LABEL, ADD_PIECE_LABEL),
        };

        let kinds: Element<'_, Message> = match raw {
            true => self.strip(draft, KINDS_LINE),
            false => {
                let mut rows = Column::new().spacing(UNIT_GAP).width(Length::Fill);

                for at in 0..draft.kinds() {
                    rows = rows.push(
                        row![text(format!("{KIND_LABEL} {at}")).size(LABEL_SIZE).width(Length::Fill), self.number(draft, KINDS_LINE, at, NUMBER_WIDTH)]
                            .spacing(GAP)
                            .align_y(Vertical::Center),
                    );
                }

                rows.push(text(KINDS_NOTE).size(NOTE_SIZE).style(text::secondary)).into()
            }
        };

        cards = cards.push(card(column![text(kinds_heading).size(HEADING_SIZE), kinds, self.resize(draft, KINDS_LINE, add_kind)].spacing(CARD_GAP).into()));

        if draft.lines.len() > TEMPLATE_LINE {
            let mut template = column![text(template_heading).size(HEADING_SIZE), self.strip(draft, TEMPLATE_LINE), self.resize(draft, TEMPLATE_LINE, add_piece)].spacing(CARD_GAP);

            if !raw {
                template = template.push(text(TEMPLATE_NOTE).size(NOTE_SIZE).style(text::secondary));
            }

            cards = cards.push(card(template.into()));
        }

        for map in &draft.maps {
            let (heading, id_label) = match raw {
                true => (RAW_ID_LABEL.to_owned(), RAW_ID_LABEL),
                false => (format!("{MAP_HEADING} {}", cell(draft.line(map.head), 0)), ID_LABEL),
            };

            let mut body = column![
                text(heading).size(HEADING_SIZE),
                row![text(id_label).size(LABEL_SIZE).width(Length::Fill), self.number(draft, map.head, 0, NUMBER_WIDTH)].spacing(GAP).align_y(Vertical::Center)
            ]
            .spacing(CARD_GAP);

            for (index, stack) in map.stacks.iter().enumerate() {
                let title = match raw {
                    true => format!("{RAW_ENTRY_LABEL} {}", index + 1),
                    false => format!("{SECTION_LABEL} {}: {} {STAGES_LABEL}", letter(index), draft.stages(stack)),
                };

                let mut values = Column::new().spacing(UNIT_GAP).width(Length::Fill);

                for (at, label) in SCATTER_LABELS.iter().enumerate() {
                    let label = match raw {
                        true => format!("{RAW_VALUE_LABEL} {}", at + 1),
                        false => (*label).to_owned(),
                    };

                    values = values.push(
                        row![text(label).size(LABEL_SIZE).width(Length::Fill), self.number(draft, stack.scatter, at, NUMBER_WIDTH)]
                            .spacing(GAP)
                            .align_y(Vertical::Center),
                    );
                }

                body = body.push(card(
                    column![text(title).size(LABEL_SIZE), self.strip(draft, stack.pieces), self.resize(draft, stack.pieces, add_piece), values]
                        .spacing(CARD_GAP)
                        .into(),
                ));
            }

            cards = cards.push(card(body.into()));
        }

        let note = match raw {
            true => RAW_NOTE,
            false => NOTE,
        };

        cards.push(text(note).size(NOTE_SIZE).style(text::secondary)).into()
    }
}

fn card(body: Element<'_, Message>) -> Element<'_, Message> {
    container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let ours = reader.text(&held);
    let theirs = reader.text(&plan.game);

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{maps, Stack};

    #[test]
    fn a_single_cell_row_starts_a_map_and_other_rows_pair_up_under_it() {
        let lines: Vec<String> = ["1,2", "0,1", "7", "0,1,1", "3,5,100", "0,1", "3,3,200", "8", "0,1,1", "3,5,100", "9", "0,1"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        let found = maps(&lines);

        assert_eq!(found.len(), 3);
        assert_eq!(found[0].stacks, [Stack { pieces: 3, scatter: 4 }, Stack { pieces: 5, scatter: 6 }]);
        assert_eq!(found[1].head, 7);
        // A file may end before the last entry's three values.
        assert_eq!(found[2].stacks, [Stack { pieces: 11, scatter: 12 }]);
    }
}
