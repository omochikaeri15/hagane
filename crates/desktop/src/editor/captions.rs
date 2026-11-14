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

pub(crate) use foundry::systems::editor::captions::*;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(460.0, 520.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Captions, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const HEADING_SIZE: f32 = 14.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const RAW_WIDTH: f32 = 280.0;
const LABEL_WIDTH: f32 = 120.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This entry has no line in the file";
const CELL_LABEL: &str = "Cell";
const COMMENT: &str = "//";

const SPAN_LABEL: &str = "Line";

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    layout: Layout,
    key: u32,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) layout: Layout,
    pub(super) key: u32,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, layout, key } = seed;

    Plan { label, file, game, target_mod, values, layout, key }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn delimiter(&self) -> char {
        delimiter_of(&self.file)
    }
}

fn located(lines: &[String], layout: Layout, key: u32, delimiter: char) -> Option<usize> {
    match layout.keyed() {
        true => lines.iter().position(|line| keyed(line, delimiter) == Some(key)),
        false => usize::try_from(key).ok().filter(|at| *at < lines.len()),
    }
}

fn entry(body: &str, layout: Layout, key: u32, delimiter: char) -> Option<String> {
    match layout.keyed() {
        true => body.lines().find(|line| keyed(line, delimiter) == Some(key)).map(str::to_owned),
        false => body.lines().nth(usize::try_from(key).ok()?).map(str::to_owned),
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Line {
    cells: Vec<String>,
    trailing: bool,
    tail: String,
}

impl Line {
    fn split(raw: &str, delimiter: char, comments: bool) -> Line {
        let (data, tail) = match raw.find(COMMENT).filter(|_| comments) {
            Some(at) => {
                let (data, comment) = raw.split_at(at);
                let kept = data.trim_end();

                (kept, format!("{}{comment}", &data[kept.len()..]))
            }
            None => (raw, String::new()),
        };

        let trailing = data.ends_with(delimiter);
        let body = data.strip_suffix(delimiter).unwrap_or(data);

        Line { cells: body.split(delimiter).map(str::to_owned).collect(), trailing, tail }
    }

    fn render(&self, delimiter: char) -> String {
        let mut line = self.cells.join(&delimiter.to_string());

        if self.trailing {
            line.push(delimiter);
        }

        line.push_str(&self.tail);

        line
    }

    fn cell(&self, at: usize) -> &str {
        self.cells.get(at).map_or("", String::as_str)
    }
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    row: Option<usize>,
    line: Line,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Caption editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let mut draft = Draft { plan, read_from, stamp, lines, row: None, line: Line::default(), failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let delimiter = self.plan.delimiter();

        let layout = self.plan.layout;

        self.row = located(&self.lines, layout, self.plan.key, delimiter);
        self.line = self.row.and_then(|at| self.lines.get(at)).map_or_else(Line::default, |raw| Line::split(raw, delimiter, layout.comments()));
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn write(&mut self, at: usize, typed: &str) -> bool {
        let delimiter = self.plan.delimiter();

        let layout = self.plan.layout;

        if (layout.keyed() && at == KEY_CELL) || at >= layout.width().max(self.line.cells.len()) || typed.contains(delimiter) || typed.contains('\n') {
            return false;
        }

        let Some(row) = self.row else {
            return false;
        };

        while self.line.cells.len() <= at {
            self.line.cells.push(String::new());
        }

        let Some(cell) = self.line.cells.get_mut(at) else {
            return false;
        };

        typed.clone_into(cell);

        let rendered = self.line.render(delimiter);

        let Some(slot) = self.lines.get_mut(row) else {
            return false;
        };

        if *slot == rendered {
            return false;
        }

        *slot = rendered;

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Caption editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.layout, self.plan.key, self.plan.delimiter()) else {
            warn!(path = %self.plan.game.display(), "Caption editor found no vanilla copy of this entry");

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
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Caption editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Caption editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Caption editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(usize, String),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<(usize, String)>,
    confirm: Slot<()>,
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
            Message::Typed(at, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                let changed = draft.write(at, &typed);

                self.typed = Some((at, typed));

                if changed && !draft.persist(vfs) {
                    return self.failed.set((), Message::FailureExpired);
                }

                Task::none()
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

    fn typed_or(&self, at: usize, current: &str) -> String {
        match &self.typed {
            Some((held, typed)) if *held == at => typed.clone(),
            _ => current.to_owned(),
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

    fn input<'a>(&'a self, at: usize, current: &str, width: Length) -> Element<'a, Message> {
        text_input("", &self.typed_or(at, current))
            .on_input(move |typed| Message::Typed(at, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(width)
            .style(theme::rounded_input)
            .into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut rows = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for at in 0..draft.plan.layout.width().max(draft.line.cells.len()) {
            let current = draft.line.cell(at);
            let label = text(format!("{CELL_LABEL} {}", at + 1)).size(LABEL_SIZE).width(Length::Fill);

            let control: Element<'_, Message> = match draft.plan.layout.keyed() && at == KEY_CELL {
                true => text(current.to_owned()).size(INPUT_SIZE).width(Length::Fixed(RAW_WIDTH)).into(),
                false => self.input(at, current, Length::Fixed(RAW_WIDTH)),
            };

            rows = rows.push(row![label, control].spacing(GAP).align_y(Vertical::Center));
        }

        rows.into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let layout = draft.plan.layout;
        let mut cards = Column::new().spacing(CARD_GAP).width(Length::Fill);

        if let Some((first, count)) = layout.span() {
            let mut rows = column![text(layout.title()).size(HEADING_SIZE)].spacing(CARD_GAP);

            for at in first..first + count {
                rows = rows.push(
                    row![
                        text(format!("{SPAN_LABEL} {}", at - first + 1)).size(LABEL_SIZE).width(Length::Fixed(LABEL_WIDTH)),
                        self.input(at, draft.line.cell(at), Length::Fill)
                    ]
                    .spacing(GAP)
                    .align_y(Vertical::Center),
                );
            }

            cards = cards.push(container(rows).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined));
        }

        for group in layout.groups() {
            let mut rows = column![text(group.heading).size(HEADING_SIZE)].spacing(CARD_GAP);

            for (at, label) in group.cells {
                rows = rows.push(
                    row![text(*label).size(LABEL_SIZE).width(Length::Fixed(LABEL_WIDTH)), self.input(*at, draft.line.cell(*at), Length::Fill)]
                        .spacing(GAP)
                        .align_y(Vertical::Center),
                );
            }

            cards = cards.push(container(rows).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined));
        }

        cards.push(text(layout.note()).size(NOTE_SIZE).style(text::secondary)).into()
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let delimiter = plan.delimiter();
    let ours = reader.text(&held).and_then(|body| entry(&body, plan.layout, plan.key, delimiter));
    let theirs = reader.text(&plan.game).and_then(|body| entry(&body, plan.layout, plan.key, delimiter));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{keyed, Line};

    #[test]
    fn a_text_line_rewrites_one_cell_and_keeps_its_closing_divider() {
        let raw = "1|Slow Beam||Style text|";
        let mut line = Line::split(raw, '|', false);

        assert_eq!(keyed(raw, '|'), Some(1));
        assert_eq!(line.render('|'), raw);

        line.cells[2] = "Foundation text".to_owned();
        assert_eq!(line.render('|'), "1|Slow Beam|Foundation text|Style text|");
    }

    #[test]
    fn a_note_after_the_cells_rides_along_untouched() {
        let raw = "Tap here|to begin|\u{ff20}|\t\t\t//a note";
        let mut line = Line::split(raw, '|', true);

        assert_eq!(line.cells, ["Tap here", "to begin", "\u{ff20}"]);
        assert_eq!(line.render('|'), raw);

        line.cells[1] = "to start".to_owned();
        assert_eq!(line.render('|'), "Tap here|to start|\u{ff20}|\t\t\t//a note");
    }
}
