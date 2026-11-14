use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::common::region;
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(440.0, 420.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Evolve, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const HEADING_SIZE: f32 = 14.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const RAW_WIDTH: f32 = 260.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This cat has no line in the file";
const TRUE_HEADING: &str = "True Form";
const ULTRA_HEADING: &str = "Ultra Form";
const NOTE: &str = "Up to three lines each, shown when the cat can evolve into that form\nLeave a line empty to hide it";
const LINE_LABEL: &str = "Line";
const CELL_LABEL: &str = "Cell";
const COMMENT: &str = "//";
const EMPTY: &str = "\u{ff20}";
const ASCII_EMPTY: &str = "@";

const TRUE_CELLS: [usize; 3] = [0, 1, 2];
const ULTRA_CELLS: [usize; 3] = [4, 5, 6];
const WIDTH: usize = 7;

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    line: usize,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) line: usize,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, line } = seed;

    Plan { label, file, game, target_mod, values, line }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn delimiter(&self) -> char {
        region::text_separator(&self.file).char()
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Line {
    cells: Vec<String>,
    trailing: bool,
    tail: String,
}

impl Line {
    fn split(raw: &str, delimiter: char) -> Line {
        let (data, comment) = match raw.find(COMMENT) {
            Some(at) => raw.split_at(at),
            None => (raw, ""),
        };

        let kept = data.trim_end();
        let tail = format!("{}{comment}", &data[kept.len()..]);
        let trailing = kept.ends_with(delimiter);
        let body = kept.strip_suffix(delimiter).unwrap_or(kept);
        let cells: Vec<String> = body.split(delimiter).map(str::to_owned).collect();

        Line { cells, trailing, tail }
    }

    fn render(&self, delimiter: char) -> String {
        let mut line = self.cells.join(&delimiter.to_string());

        if self.trailing {
            line.push(delimiter);
        }

        line.push_str(&self.tail);

        line
    }

    fn shown(&self, at: usize) -> &str {
        let cell = self.cells.get(at).map_or("", String::as_str);

        match cell.trim() {
            EMPTY | ASCII_EMPTY => "",
            _ => cell,
        }
    }
}

fn entry(body: &str, line: usize) -> Option<String> {
    body.lines().nth(line).map(str::to_owned)
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    row: Line,
    present: bool,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Evolution text editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let mut draft = Draft { plan, read_from, stamp, lines, row: Line::default(), present: false, failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let delimiter = self.plan.delimiter();

        self.present = self.plan.line < self.lines.len();
        self.row = self.lines.get(self.plan.line).map_or_else(Line::default, |raw| Line::split(raw, delimiter));
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn write(&mut self, at: usize, typed: &str, raw: bool) -> bool {
        let delimiter = self.plan.delimiter();

        if !self.present || typed.contains(delimiter) || typed.contains('\n') {
            return false;
        }

        let value = match (raw, typed.trim().is_empty()) {
            (false, true) => EMPTY.to_owned(),
            _ => typed.to_owned(),
        };

        if at >= WIDTH {
            return false;
        }

        while self.row.cells.len() <= at {
            self.row.cells.push(EMPTY.to_owned());
        }

        let Some(cell) = self.row.cells.get_mut(at) else {
            return false;
        };

        *cell = value;

        let rendered = self.row.render(delimiter);

        let Some(slot) = self.lines.get_mut(self.plan.line) else {
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
            warn!(path = %self.plan.game.display(), "Evolution text editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.line) else {
            warn!(path = %self.plan.game.display(), "Evolution text editor found no vanilla copy of this cat");

            return false;
        };

        if let Some(slot) = self.lines.get_mut(self.plan.line) {
            *slot = original;
        }

        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Evolution text editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Evolution text editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Evolution text editor could not write the file: {}", err);
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

                let raw = draft.plan.values == EditorMode::Raw;
                let changed = draft.write(at, &typed, raw);

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

        for at in 0..WIDTH.max(draft.row.cells.len()) {
            let current = draft.row.cells.get(at).map_or("", String::as_str);

            rows = rows.push(
                row![text(format!("{CELL_LABEL} {}", at + 1)).size(LABEL_SIZE).width(Length::Fill), self.input(at, current, Length::Fixed(RAW_WIDTH))]
                    .spacing(GAP)
                    .align_y(Vertical::Center),
            );
        }

        rows.into()
    }

    fn form<'a>(&'a self, draft: &'a Draft, heading: &'a str, cells: [usize; 3]) -> Element<'a, Message> {
        let mut rows = column![text(heading).size(HEADING_SIZE)].spacing(CARD_GAP);

        for (number, at) in cells.into_iter().enumerate() {
            rows = rows.push(
                row![text(format!("{LINE_LABEL} {}", number + 1)).size(LABEL_SIZE), self.input(at, draft.row.shown(at), Length::Fill)]
                    .spacing(GAP)
                    .align_y(Vertical::Center),
            );
        }

        container(rows).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        column![
            self.form(draft, TRUE_HEADING, TRUE_CELLS),
            self.form(draft, ULTRA_HEADING, ULTRA_CELLS),
            text(NOTE).size(NOTE_SIZE).style(text::secondary),
        ]
        .spacing(CARD_GAP)
        .width(Length::Fill)
        .into()
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| entry(&body, plan.line));
    let theirs = reader.text(&plan.game).and_then(|body| entry(&body, plan.line));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{Line, EMPTY};

    #[test]
    fn a_cat_line_keeps_its_divider_spacing_and_note() {
        let raw = "A|B|\u{ff20}|\u{ff20}|C|\u{ff20}|\u{ff20}|\t\t\t//note";
        let mut line = Line::split(raw, '|');

        assert_eq!(line.render('|'), raw);
        assert_eq!(line.shown(2), "");
        assert_eq!(line.cells[3], EMPTY, "the divider between the two forms");

        line.cells[2] = "D".to_owned();
        assert_eq!(line.render('|'), "A|B|D|\u{ff20}|C|\u{ff20}|\u{ff20}|\t\t\t//note");
    }
}
