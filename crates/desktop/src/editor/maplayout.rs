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

const SIZE: Size = Size::new(440.0, 540.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::MapLayout, SIZE);

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
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const EMPTY_LABEL: &str = "This file has no layout rows";
const COLOR_HEADING: &str = "Ground Color";
const COLOR_LABELS: [&str; 3] = ["Red", "Green", "Blue"];
const SECTION_LABEL: &str = "Section";
const STAGES_LABEL: &str = "Stages";
const STAGE_LABEL: &str = "Stage";
const COUNT_LABEL: &str = "Stage Count";
const X_LABEL: &str = "X";
const Y_LABEL: &str = "Y";
const ADD_LABEL: &str = "Add Stage Point";
const REMOVE_LABEL: &str = "Remove";
const NOTE: &str = "Each section is one map's path of stage points\nThe game counts a map's stages from its section, so adding or removing a point changes that count";
const RAW_NOTE: &str = "A section's first cell is how many point rows follow it";
const DELIMITER: char = ',';
const POINT_TAIL: &str = ",";
const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";

const COLOR_CELLS: usize = 3;
const COUNT_CELL: usize = 0;
const X_CELL: usize = 0;
const Y_CELL: usize = 1;

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

#[derive(Clone, PartialEq, Debug)]
struct Section {
    head: usize,
    points: Vec<usize>,
}

fn cell(line: &str, at: usize) -> &str {
    line.split(DELIMITER).nth(at).map_or("", str::trim)
}

fn number(line: &str, at: usize) -> Option<i64> {
    cell(line, at).parse().ok()
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

fn sections(lines: &[String]) -> Vec<Section> {
    let mut found = Vec::new();
    let mut at = 1;

    while let Some(count) = lines.get(at).and_then(|line| number(line, COUNT_CELL)).filter(|count| *count > 0) {
        let count = usize::try_from(count).unwrap_or_default();
        let points: Vec<usize> = (at + 1..=at + count).filter(|point| *point < lines.len()).collect();

        found.push(Section { head: at, points });
        at += 1 + count;
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
    sections: Vec<Section>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Map layout editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let sections = sections(&lines);

        Some(Draft { plan, read_from, stamp, lines, sections, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn line(&self, at: usize) -> &str {
        self.lines.get(at).map_or("", String::as_str)
    }

    fn write(&mut self, line: usize, at: usize, value: &str) -> bool {
        let Some(slot) = self.lines.get_mut(line) else {
            return false;
        };

        let fresh = replaced(slot, at, value);

        if *slot == fresh {
            return false;
        }

        *slot = fresh;
        self.sections = sections(&self.lines);

        true
    }

    fn add(&mut self, section: usize) -> bool {
        let Some(held) = self.sections.get(section).cloned() else {
            return false;
        };

        let source = held.points.last().map_or(held.head, |point| *point);
        let x = number(self.line(source), X_CELL).filter(|_| source != held.head).unwrap_or_default();
        let y = number(self.line(source), Y_CELL).filter(|_| source != held.head).unwrap_or_default();
        let count = held.points.len() + 1;

        let head = replaced(self.line(held.head), COUNT_CELL, &count.to_string());

        if let Some(slot) = self.lines.get_mut(held.head) {
            *slot = head;
        }

        self.lines.insert(source + 1, format!("{x}{DELIMITER}{y}{POINT_TAIL}"));
        self.sections = sections(&self.lines);

        true
    }

    fn remove(&mut self, section: usize, point: usize) -> bool {
        let Some(held) = self.sections.get(section).cloned() else {
            return false;
        };

        let Some(&line) = held.points.get(point) else {
            return false;
        };

        if held.points.len() <= 1 {
            return false;
        }

        let head = replaced(self.line(held.head), COUNT_CELL, &(held.points.len() - 1).to_string());

        if let Some(slot) = self.lines.get_mut(held.head) {
            *slot = head;
        }

        self.lines.remove(line);
        self.sections = sections(&self.lines);

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Map layout editor could not read the vanilla file");

            return false;
        };

        self.lines = common::scrub(&bytes).lines().map(str::to_owned).collect();
        self.sections = sections(&self.lines);

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Map layout editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Map layout editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Map layout editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Intent {
    Remove(usize, usize),
    Sync,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(usize, usize, String),
    Add(usize),
    Remove(usize, usize),
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
                let changed = numeric(&typed) && self.draft.as_mut().is_some_and(|draft| draft.write(line, at, &typed));

                self.typed = Some((line, at, typed));

                self.settle(changed, vfs)
            }
            Message::Add(section) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.add(section));

                self.settle(changed, vfs)
            }
            Message::Remove(section, point) => {
                if !self.confirm.take(&Intent::Remove(section, point)) {
                    return self.confirm.set(Intent::Remove(section, point), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.remove(section, point));

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

    fn number<'a>(&'a self, draft: &'a Draft, line: usize, at: usize) -> Element<'a, Message> {
        text_input("", &self.typed_or(line, at, cell(draft.line(line), at)))
            .on_input(move |typed| Message::Typed(line, at, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fixed(NUMBER_WIDTH))
            .style(theme::rounded_input)
            .into()
    }

    fn listing<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let raw = draft.plan.values == EditorMode::Raw;
        let mut cards = Column::new().spacing(CARD_GAP).width(Length::Fill);

        let mut color = row![].spacing(GAP).align_y(Vertical::Center);

        for (at, label) in COLOR_LABELS.iter().enumerate().take(COLOR_CELLS) {
            color = color.push(text(*label).size(LABEL_SIZE)).push(self.number(draft, 0, at));
        }

        cards = cards.push(card(column![text(COLOR_HEADING).size(HEADING_SIZE), color].spacing(CARD_GAP).into()));

        for (index, section) in draft.sections.iter().enumerate() {
            let heading = format!("{SECTION_LABEL} {}: {} {STAGES_LABEL}", letter(index), section.points.len());
            let mut rows = Column::new().spacing(UNIT_GAP).width(Length::Fill);

            if raw {
                rows = rows.push(row![text(COUNT_LABEL).size(LABEL_SIZE).width(Length::Fill), self.number(draft, section.head, COUNT_CELL)].spacing(GAP).align_y(Vertical::Center));
            }

            for (position, line) in section.points.iter().enumerate() {
                let armed = self.confirm.armed_for(&Intent::Remove(index, position));

                let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                    .padding(SMALL_PADDING)
                    .style(theme::danger_button)
                    .on_press_maybe((section.points.len() > 1).then_some(Message::Remove(index, position)));

                rows = rows.push(
                    row![
                        text(format!("{STAGE_LABEL} {}", position + 1)).size(LABEL_SIZE).width(Length::Fill),
                        text(X_LABEL).size(NOTE_SIZE),
                        self.number(draft, *line, X_CELL),
                        text(Y_LABEL).size(NOTE_SIZE),
                        self.number(draft, *line, Y_CELL),
                        remove
                    ]
                    .spacing(GAP)
                    .align_y(Vertical::Center),
                );
            }

            let add = button(theme::centered_text(ADD_LABEL).size(LABEL_SIZE))
                .padding(SMALL_PADDING)
                .style(theme::primary_button)
                .on_press(Message::Add(index));

            cards = cards.push(card(column![text(heading).size(HEADING_SIZE), rows, add].spacing(CARD_GAP).into()));
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
    use super::{replaced, sections};

    #[test]
    fn a_section_reads_its_count_of_points_and_stops_at_the_note() {
        let lines: Vec<String> = ["76,93,85,0,\t//color", "2,\t//A", "1,2,\t//1", "3,4,\t//2", "1,\t//B", "5,6,", "//x,y"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        let found = sections(&lines);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].points, [2, 3]);
        assert_eq!(found[1].head, 4);
        assert_eq!(replaced("874,914,\t\t//1", 1, "900"), "874,900,\t\t//1");
    }
}
