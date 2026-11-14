use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, toggler, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::chapter::treasure::{TreasureEffect, GROUPS, GROUP_STAGES};
use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 500.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Treasure, SIZE);

pub(super) const TITLE: &str = "Treasure Set";
pub(super) const FOLDER: &str = "Treasures";

pub(super) const ARCS: [(usize, &str); 5] = [
    (0, "Empire of Cats"),
    (4, "Into the Future"),
    (7, "Cats of the Cosmos Ch. 1"),
    (8, "Cats of the Cosmos Ch. 2"),
    (9, "Cats of the Cosmos Ch. 3"),
];

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const INPUT_WIDTH: f32 = 170.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This set is past the end of the file";
const COUNT_LABEL: &str = "Treasures in Set";
const STAGES_LABEL: &str = "Stages";
const IMAGE_LABEL: &str = "Image";
const VALUE_LABEL: &str = "Effect Value";
const EFFECT_LABEL: &str = "Effect";
const CHAPTER_LABEL: &str = "Only counts in its own chapter";
const NOTE_LABEL: &str = "The value's unit depends on the effect, as the file's note for this set says";
const LIST_JOINT: &str = ", ";
const COMMENT: &str = "//";
const DELIMITER: char = ',';
const TERMINATOR: i64 = -1;

const COUNT_BLOCK: usize = 0;
const STAGE_BLOCK: usize = 1;
const IMAGE_BLOCK: usize = 2;
const EFFECT_BLOCK: usize = 3;
const BLOCKS: usize = 4;

const VALUE_CELL: usize = 0;
const EFFECT_CELL: usize = 1;
const CHAPTER_CELL: usize = 2;

const RAW_LABELS: [&str; BLOCKS] = ["count", "stages", "image", "effect"];

const EFFECTS: [(i32, &str); 25] = [
    (0, "Base Health"),
    (1, "Cannon Recharge"),
    (2, "Cannon Power"),
    (3, "Worker Cat Efficiency"),
    (4, "Wallet Capacity"),
    (5, "Money From Enemies"),
    (6, "Experience"),
    (7, "Energy Recovery"),
    (8, "Max Energy"),
    (9, "Cat Health"),
    (10, "Cat Attack"),
    (11, "Cat Recharge"),
    (12, "Red Fruit"),
    (13, "Black Fruit"),
    (14, "Floating Fruit"),
    (15, "Angel Fruit"),
    (16, "Alien Weakening"),
    (17, "Cannon Range Recharge"),
    (18, "Starred Alien Weakening"),
    (19, "Metal Fruit"),
    (20, "Zombie Fruit"),
    (21, "Alien Fruit"),
    (22, "Starred Alien Type 2"),
    (23, "Starred Alien Type 3"),
    (24, "Starred Alien Type 4"),
];

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    group: usize,
    items: Vec<String>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) group: usize,
    pub(super) items: Vec<String>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, group, items } = seed;

    Plan { label, file, game, target_mod, values, group, items }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn item(&self, stage: i64) -> Option<&str> {
        usize::try_from(stage).ok().and_then(|at| self.items.get(at)).map(String::as_str).filter(|name| !name.is_empty())
    }
}

fn effect_name(effect: TreasureEffect) -> Option<&'static str> {
    EFFECTS.iter().find(|(id, _)| *id == effect.id()).map(|(_, name)| *name)
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Line {
    cells: Vec<String>,
    trailing: bool,
    tail: String,
}

impl Line {
    fn split(raw: &str) -> Line {
        let (data, comment) = match raw.find(COMMENT) {
            Some(at) => raw.split_at(at),
            None => (raw, ""),
        };

        let kept = data.trim_end();
        let tail = format!("{}{comment}", &data[kept.len()..]);
        let trailing = kept.ends_with(DELIMITER);
        let body = kept.strip_suffix(DELIMITER).unwrap_or(kept);

        let cells = match body.is_empty() {
            true => Vec::new(),
            false => body.split(DELIMITER).map(str::to_owned).collect(),
        };

        Line { cells, trailing, tail }
    }

    fn render(&self) -> String {
        let mut line = self.cells.join(&DELIMITER.to_string());

        if self.trailing {
            line.push(DELIMITER);
        }

        line.push_str(&self.tail);

        line
    }

    fn number(&self, at: usize) -> i64 {
        self.cells.get(at).and_then(|cell| cell.trim().parse().ok()).unwrap_or(0)
    }

    fn note(&self) -> &str {
        self.tail.trim_start().strip_prefix(COMMENT).map_or("", str::trim)
    }

    fn set(&mut self, at: usize, value: String) {
        while self.cells.len() <= at {
            self.cells.push(0.to_string());
        }

        if let Some(cell) = self.cells.get_mut(at) {
            *cell = value;
        }
    }
}

fn line_of(block: usize, group: usize) -> usize {
    block * GROUPS + group
}

fn entry(body: &str, group: usize) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    let held: Option<Vec<&str>> = (0..BLOCKS).map(|block| lines.get(line_of(block, group)).copied()).collect();

    held.map(|held| held.join("\n"))
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    rows: [Line; BLOCKS],
    present: bool,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Treasure editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let mut draft = Draft { plan, read_from, stamp, lines, rows: Default::default(), present: false, failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let group = self.plan.group;

        self.present = (0..BLOCKS).all(|block| line_of(block, group) < self.lines.len());
        self.rows = std::array::from_fn(|block| self.lines.get(line_of(block, group)).map_or_else(Line::default, |raw| Line::split(raw)));
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn stages(&self) -> Vec<i64> {
        self.rows[STAGE_BLOCK]
            .cells
            .iter()
            .filter_map(|cell| cell.trim().parse::<i64>().ok())
            .take(GROUP_STAGES)
            .take_while(|stage| *stage != TERMINATOR)
            .collect()
    }

    fn stage(&mut self, block: usize) -> bool {
        if !self.present {
            return false;
        }

        let at = line_of(block, self.plan.group);
        let rendered = self.rows[block].render();

        let Some(slot) = self.lines.get_mut(at) else {
            return false;
        };

        if *slot == rendered {
            return false;
        }

        *slot = rendered;

        true
    }

    fn write(&mut self, block: usize, cell: usize, value: String) -> bool {
        self.rows[block].set(cell, value);

        self.stage(block)
    }

    fn write_stages(&mut self, stages: &[i64]) -> bool {
        let mut cells: Vec<String> = stages.iter().take(GROUP_STAGES).map(i64::to_string).collect();

        cells.push(TERMINATOR.to_string());

        self.rows[STAGE_BLOCK].cells = cells;

        self.stage(STAGE_BLOCK)
    }

    fn write_raw(&mut self, block: usize, typed: &str) -> bool {
        let parsed: Option<Vec<String>> = typed
            .split(DELIMITER)
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(|cell| cell.parse::<i64>().ok().map(|number| number.to_string()))
            .collect();

        let Some(cells) = parsed else {
            return false;
        };

        self.rows[block].cells = cells;

        self.stage(block)
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Treasure editor could not read the vanilla file");

            return false;
        };

        let vanilla: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        for block in 0..BLOCKS {
            let at = line_of(block, self.plan.group);

            if let (Some(original), Some(slot)) = (vanilla.get(at), self.lines.get_mut(at)) {
                original.clone_into(slot);
            }
        }

        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Treasure editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Treasure editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Treasure editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Choice {
    raw: i32,
    label: &'static str,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Count,
    Stages,
    Image,
    Value,
    Raw(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(Field, String),
    Effect(i32),
    Chapter(bool),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<(Field, String)>,
    confirm: Slot<()>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

fn digits(typed: &str) -> bool {
    let body = typed.strip_prefix('-').unwrap_or(typed);

    body.chars().all(|glyph| glyph.is_ascii_digit())
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

                let number = |typed: &str| (!typed.is_empty() && digits(typed)).then(|| typed.parse::<i64>().ok()).flatten();

                let changed = match field {
                    Field::Count => number(&typed).is_some_and(|value| draft.write(COUNT_BLOCK, 0, value.max(0).to_string())),
                    Field::Image => number(&typed).is_some_and(|value| draft.write(IMAGE_BLOCK, 0, value.to_string())),
                    Field::Value => number(&typed).is_some_and(|value| draft.write(EFFECT_BLOCK, VALUE_CELL, value.to_string())),
                    Field::Stages => {
                        let parsed: Option<Vec<i64>> = typed
                            .split(DELIMITER)
                            .map(str::trim)
                            .filter(|cell| !cell.is_empty())
                            .map(|cell| cell.parse::<i64>().ok().filter(|stage| *stage >= 0))
                            .collect();

                        parsed.is_some_and(|stages| draft.write_stages(&stages))
                    }
                    Field::Raw(block) => draft.write_raw(block, &typed),
                };

                self.typed = Some((field, typed));

                self.settle(changed, vfs)
            }
            Message::Effect(raw) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(EFFECT_BLOCK, EFFECT_CELL, raw.to_string()));

                self.settle(changed, vfs)
            }
            Message::Chapter(only) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(EFFECT_BLOCK, CHAPTER_CELL, i32::from(only).to_string()));

                self.settle(changed, vfs)
            }
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
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
        let listing: Element<'_, Message> = match (draft.present, draft.plan.values) {
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

        let label = match (self.failed.is_set() || draft.failed, self.confirm.is_set()) {
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

    fn input<'a>(&'a self, field: Field, current: String, width: Length) -> Element<'a, Message> {
        text_input("", &self.typed_or(field, current))
            .on_input(move |typed| Message::Typed(field, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(width)
            .style(theme::rounded_input)
            .into()
    }

    fn line<'a>(&'a self, label: &'a str, control: Element<'a, Message>) -> Element<'a, Message> {
        row![text(label).size(LABEL_SIZE).width(Length::Fill), control].spacing(GAP).align_y(Vertical::Center).into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut rows = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for (block, label) in RAW_LABELS.iter().enumerate() {
            let current = draft.rows[block].cells.iter().map(|cell| cell.trim()).collect::<Vec<_>>().join(LIST_JOINT);

            rows = rows.push(self.line(label, self.input(Field::Raw(block), current, Length::Fixed(INPUT_WIDTH))));
        }

        rows.into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let plan = &draft.plan;
        let stages = draft.stages();
        let listed: Vec<String> = stages.iter().map(i64::to_string).collect();
        let named: Vec<&str> = stages.iter().filter_map(|stage| plan.item(*stage)).collect();

        let options: Vec<Choice> = EFFECTS.iter().map(|(raw, label)| Choice { raw: *raw, label }).collect();
        let current = i32::try_from(draft.rows[EFFECT_BLOCK].number(EFFECT_CELL)).unwrap_or(i32::MIN);
        let chosen = options.iter().find(|choice| choice.raw == current).cloned();
        let placeholder = effect_name(TreasureEffect::from(current)).map_or_else(|| current.to_string(), str::to_owned);

        let effect = pick_list(options, chosen, |picked: Choice| Message::Effect(picked.raw))
            .placeholder(placeholder)
            .text_size(INPUT_SIZE)
            .width(Length::Fixed(INPUT_WIDTH))
            .style(theme::combo_box);

        let only = draft.rows[EFFECT_BLOCK].number(CHAPTER_CELL) != 0;

        let chapter = toggler(only).label(CHAPTER_LABEL).text_size(LABEL_SIZE).on_toggle(Message::Chapter).style(theme::ios_toggle);

        let note = draft.rows[EFFECT_BLOCK].note();

        let set = card(
            column![
                self.line(COUNT_LABEL, self.input(Field::Count, draft.rows[COUNT_BLOCK].number(0).to_string(), Length::Fixed(INPUT_WIDTH))),
                column![
                    text(STAGES_LABEL).size(LABEL_SIZE),
                    self.input(Field::Stages, listed.join(LIST_JOINT), Length::Fill),
                    text(named.join(LIST_JOINT)).size(NOTE_SIZE).style(text::secondary),
                ]
                .spacing(2),
                self.line(IMAGE_LABEL, self.input(Field::Image, draft.rows[IMAGE_BLOCK].number(0).to_string(), Length::Fixed(INPUT_WIDTH))),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        let bonus = card(
            column![
                self.line(EFFECT_LABEL, effect.into()),
                self.line(VALUE_LABEL, self.input(Field::Value, draft.rows[EFFECT_BLOCK].number(VALUE_CELL).to_string(), Length::Fixed(INPUT_WIDTH))),
                chapter,
                text(if note.is_empty() { NOTE_LABEL.to_owned() } else { format!("{NOTE_LABEL}: {note}") }).size(NOTE_SIZE).style(text::secondary),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        column![set, bonus].spacing(CARD_GAP).width(Length::Fill).into()
    }
}

fn card(body: Element<'_, Message>) -> Element<'_, Message> {
    container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| entry(&body, plan.group));
    let theirs = reader.text(&plan.game).and_then(|body| entry(&body, plan.group));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::Line;

    #[test]
    fn a_line_keeps_its_spacing_and_note_when_a_cell_changes() {
        let mut line = Line::split("100,3,0,        //0: a note");

        assert_eq!(line.render(), "100,3,0,        //0: a note");
        assert_eq!(line.note(), "0: a note");

        line.set(0, "250".to_owned());
        assert_eq!(line.render(), "250,3,0,        //0: a note");
    }
}
