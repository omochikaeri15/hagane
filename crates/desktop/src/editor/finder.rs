use iced::alignment::Vertical;
use iced::widget::{button, column, container, operation, responsive, row, scrollable, space, text, text_input, Column, Id};
use iced::{Color, Element, Length, Padding, Size, Task, Theme};
use rustc_hash::{FxHashMap, FxHashSet};

use foundry::domains::settings::EditorMode;
use foundry::systems::editor::finder::{words, Entry, Filter, Line};

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::common::fonts;
use crate::common::row_window::{self, RowWindow};
use crate::widget::{branches, open_mark, popup, smooth_scroll, tinted_row, Branches, Guide, Tint, Tracer};

use super::{Mark, Pick};

pub(crate) use foundry::systems::editor::finder::Catalog;

const SIZE: Size = Size::new(440.0, 480.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::ModEdit, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const NOTE_SIZE: f32 = 13.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 6.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const LIST_GAP: f32 = 3.0;
const LIST_INSET: f32 = theme::CARD_BORDER_WIDTH + LIST_GAP;

const ROW_HEIGHT: f32 = 24.0;
const ROW_SIZE: f32 = 13.0;
const ROW_PADDING: f32 = 6.0;
const INDENT: f32 = 12.0;
const CHAR_WIDTH: f32 = ROW_SIZE * 0.65;

const MARKER_SIZE: f32 = ROW_HEIGHT * fonts::TRIANGLE_ROW;
const MARKER_LINE_HEIGHT: f32 = ROW_HEIGHT / MARKER_SIZE;
const MARKER_WIDTH: f32 = 16.0;

const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 2.0;
const SCROLLBAR_ALLOWANCE: f32 = 14.0;
const SCROLL_TAIL: f32 = 14.0;

const FOLDER_OPEN: &str = "\u{25be}";
const FOLDER_SHUT: &str = "\u{25b8}";

const MODES: [EditorMode; 2] = [EditorMode::Raw, EditorMode::Resolved];

const SYNC_LABEL: &str = "Sync With \"game\"";
const MARK_REACH: f32 = 1600.0;
const MARK_ALPHA: f32 = 0.3;

fn mode_slot(values: EditorMode) -> usize {
    match values {
        EditorMode::Raw => 0,
        EditorMode::Resolved => 1,
    }
}

fn list_id() -> Id {
    Id::new("finder-list")
}

fn both_ways() -> scrollable::Direction {
    let bar = || scrollable::Scrollbar::new().width(SCROLLBAR_WIDTH).margin(SCROLLBAR_MARGIN);

    scrollable::Direction::Both { vertical: bar(), horizontal: bar() }
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

fn changed(theme: &Theme) -> Color {
    Color { a: MARK_ALPHA, ..theme.palette().warning }
}

fn added(theme: &Theme) -> Color {
    Color { a: MARK_ALPHA, ..theme.palette().success }
}

fn tint(mark: Option<Mark>) -> Option<Tint> {
    match mark {
        Some(Mark::Changed) => Some(changed),
        Some(Mark::Added) => Some(added),
        Some(Mark::Same) | None => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Act {
    Edit,
    Sync,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Searched(String),
    Activate(Entry),
    Scrolled(f32),
    Edit,
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    open: bool,
    frame: popup::State,
    catalog: Catalog,
    query: String,
    filter: Filter,
    lines: Vec<Line>,
    guides: Vec<Guide>,
    expanded: FxHashSet<u32>,
    collapsed: FxHashSet<u32>,
    widest: [f32; MODES.len()],
    picked: Option<u32>,
    offset: f32,
    choosing: bool,
    resetting: bool,
    marks: FxHashMap<u32, Mark>,
    confirm: Slot<()>,
    failed: Slot<Act>,
}

impl State {
    pub(super) fn finding(&self) -> bool {
        self.open
    }

    pub(super) fn choosing(&self) -> bool {
        self.choosing
    }

    pub(super) fn open(&mut self, catalog: Option<Catalog>) {
        let Some(catalog) = catalog else {
            self.frame.update(popup::Message::Raise, POPUP);

            return;
        };

        *self = State { frame: popup::cascaded(0), filter: catalog.filter(""), catalog, open: true, ..State::default() };

        self.relayout();
    }

    pub(super) fn close(&mut self) {
        *self = State::default();
    }

    pub(super) fn rebuild(&mut self, catalog: Catalog) {
        let keyed = |ids: &FxHashSet<u32>| -> Vec<String> {
            ids.iter().filter_map(|id| self.catalog.folder_key(*id)).collect()
        };

        let (expanded, collapsed) = (keyed(&self.expanded), keyed(&self.collapsed));
        let picked = self.picked.and_then(|leaf| self.catalog.lead(leaf));

        self.catalog = catalog;

        let folders = self.catalog.folder_ids();
        let found = |keys: Vec<String>| -> FxHashSet<u32> { keys.iter().filter_map(|key| folders.get(key).copied()).collect() };

        self.expanded = found(expanded);
        self.collapsed = found(collapsed);
        self.filter = self.catalog.filter(&self.query);
        self.picked = picked.and_then(|lead| self.catalog.follow(&lead, &folders)).filter(|leaf| self.filter.keeps(*leaf));
        self.marks.clear();
        self.relayout();
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn take_choice(&mut self, target: Option<String>) -> Option<Pick> {
        if !std::mem::take(&mut self.choosing) {
            return None;
        }

        Some(super::adopted(self.catalog.pick(self.picked?)?, target?))
    }

    pub(super) fn failed(&mut self) -> Task<Message> {
        self.failed.set(Act::Edit, Message::FailureExpired)
    }

    pub(super) fn resetting(&self) -> bool {
        self.resetting
    }

    pub(super) fn take_reset(&mut self, target: Option<String>) -> Option<Pick> {
        if !std::mem::take(&mut self.resetting) {
            return None;
        }

        Some(super::adopted(self.catalog.pick(self.picked?)?, target?))
    }

    pub(super) fn reset_failed(&mut self) -> Task<Message> {
        self.failed.set(Act::Sync, Message::FailureExpired)
    }

    pub(super) fn pick(&self, leaf: u32, target: String) -> Option<Pick> {
        Some(super::adopted(self.catalog.pick(leaf)?, target))
    }

    pub(super) fn unmarked(&self) -> Vec<u32> {
        if !self.open {
            return Vec::new();
        }

        let RowWindow { range, .. } =
            row_window::compute_with(self.lines.len(), MARK_REACH, self.offset, ROW_HEIGHT, 0.0);

        let seen = self.lines.get(range).unwrap_or_default().iter().filter_map(|line| match line.entry {
            Entry::Leaf(leaf) => Some(leaf),
            Entry::Folder(_) => None,
        });

        seen.chain(self.picked).filter(|leaf| !self.marks.contains_key(leaf)).collect()
    }

    pub(super) fn marked(&mut self, marks: Vec<(u32, Mark)>) {
        self.marks.extend(marks);
    }

    pub(super) fn unmark(&mut self) {
        self.marks.clear();
    }

    fn modded(&self) -> bool {
        self.picked.and_then(|leaf| self.marks.get(&leaf)).is_some_and(|mark| *mark != Mark::Same)
    }

    fn relayout(&mut self) {
        let searching = self.filter.searching();
        let (expanded, collapsed) = (&self.expanded, &self.collapsed);

        self.lines = self.catalog.lines(&self.filter, |id| match searching {
            true => !collapsed.contains(&id),
            false => expanded.contains(&id),
        });

        let mut tracer = Tracer::default();

        self.guides = vec![Guide::default(); self.lines.len()];

        for index in (0..self.lines.len()).rev() {
            let depth = self.lines[index].depth;
            let above = index.checked_sub(1).map(|prev| self.lines[prev].depth);

            self.guides[index] = tracer.back(depth, above);
        }

        self.widest = MODES.map(|values| {
            self.lines
                .iter()
                .map(|line| {
                    let label = self.catalog.label(line.entry, values);
                    let marker = if matches!(line.entry, Entry::Folder(_)) { MARKER_WIDTH } else { 0.0 };

                    ROW_PADDING * 2.0 + INDENT * f32::from(line.depth) + marker + CHAR_WIDTH * label.len() as f32
                })
                .fold(0.0, f32::max)
        });
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.close();
                }

                Task::none()
            }
            Message::Searched(query) => {
                self.filter = self.catalog.filter(&query);
                self.query = query;
                self.collapsed.clear();
                self.picked = self.picked.filter(|leaf| self.filter.keeps(*leaf));
                self.offset = 0.0;
                self.relayout();

                operation::scroll_to(list_id(), scrollable::AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::Activate(Entry::Folder(id)) => {
                let held = match self.filter.searching() {
                    true => &mut self.collapsed,
                    false => &mut self.expanded,
                };

                if !held.remove(&id) {
                    held.insert(id);
                }

                self.relayout();

                Task::none()
            }
            Message::Activate(Entry::Leaf(leaf)) => {
                self.picked = Some(leaf);
                self.failed.clear();
                self.confirm.clear();

                Task::none()
            }
            Message::Scrolled(offset) => {
                self.offset = offset;

                Task::none()
            }
            Message::Edit => {
                self.choosing = self.picked.is_some();

                Task::none()
            }
            Message::Sync => {
                if !self.modded() {
                    return Task::none();
                }

                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
                }

                self.resetting = true;

                Task::none()
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

    pub(super) fn view(&self, window: Size, values: EditorMode) -> Option<Element<'_, Message>> {
        if !self.open {
            return None;
        }

        Some(self.frame.view(words(values).title, POPUP, window, Message::Popup, move || self.body(values), None))
    }

    fn body(&self, values: EditorMode) -> Element<'_, Message> {
        let words = words(values);

        let note = theme::centered_text(words.note).size(NOTE_SIZE).width(Length::Fill);

        let search = text_input(words.hint, &self.query)
            .on_input(Message::Searched)
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let list: Element<'_, Message> = match self.lines.is_empty() {
            true => container(theme::centered_text(words.empty).size(ROW_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => responsive(move |size: Size| self.rows(size, values)).into(),
        };

        let list = container(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(LIST_INSET)
            .style(theme::card_container_outlined);

        let failed = self.failed.armed_for(&Act::Edit);
        let label = if failed { FAILURE_LABEL } else { words.action };
        let style: theme::ButtonStyleFn = if failed { theme::danger_button } else { theme::primary_button };

        let edit = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(style)
            .on_press_maybe(self.picked.map(|_| Message::Edit));

        let label = match (self.failed.armed_for(&Act::Sync), self.confirm.is_set()) {
            (true, _) => FAILURE_LABEL,
            (false, true) => CONFIRM_LABEL,
            (false, false) => SYNC_LABEL,
        };

        let sync = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::danger_button)
            .on_press_maybe(self.modded().then_some(Message::Sync));

        column![note, search, list, edit, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn rows(&self, size: Size, values: EditorMode) -> Element<'_, Message> {
        let widest = self.widest[mode_slot(values)];
        let tail = if widest > size.width { SCROLL_TAIL } else { 0.0 };

        let RowWindow { range, pad_before, pad_after } =
            row_window::compute_with(self.lines.len(), size.height - tail, self.offset, ROW_HEIGHT, 0.0);

        let width = widest.max(size.width - SCROLLBAR_ALLOWANCE);
        let mut list = Column::with_capacity(range.len() + 3);

        if pad_before > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_before)));
        }

        for index in range {
            let Some(line) = self.lines.get(index) else {
                continue;
            };

            list = list.push(self.line(line, self.guides.get(index).copied().unwrap_or_default(), width, values));
        }

        if pad_after > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_after)));
        }

        if tail > 0.0 {
            list = list.push(space().height(Length::Fixed(tail)));
        }

        let area = scrollable(list)
            .id(list_id())
            .direction(both_ways())
            .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
            .width(Length::Fill)
            .height(Length::Fill);

        smooth_scroll(area).into()
    }

    fn line<'a>(&'a self, line: &Line, guide: Guide, width: f32, values: EditorMode) -> Element<'a, Message> {
        let label = self.catalog.label(line.entry, values);
        let picked = matches!(line.entry, Entry::Leaf(leaf) if self.picked == Some(leaf));
        let stem = branches(guide, line.depth, INDENT, ROW_HEIGHT, MARKER_SIZE);

        let mark = match line.entry {
            Entry::Leaf(leaf) => self.marks.get(&leaf).copied(),
            Entry::Folder(_) => None,
        };

        let name = text(label).size(ROW_SIZE).wrapping(text::Wrapping::None);

        let (stem, face) = match (line.entry, line.open) {
            (Entry::Folder(_), true) => (stem.opened(), row![marker(true), name]),
            (Entry::Folder(_), false) => (stem, row![marker(false), name]),
            (Entry::Leaf(_), _) => (stem.reach(MARKER_WIDTH), row![name]),
        };

        let span = Branches::span(&stem);
        let inset = Branches::inset(&stem);

        let content = container(face.align_y(Vertical::Center))
            .height(Length::Fixed(ROW_HEIGHT))
            .align_y(Vertical::Center)
            .padding(Padding::default().left(inset).right(ROW_PADDING));

        let seat = tinted_row(
            content,
            picked,
            tint(mark),
            Length::Fixed((width - ROW_PADDING - span).max(0.0)),
            Message::Activate(line.entry),
        );

        let body = row![space().width(Length::Fixed(ROW_PADDING)), stem, seat].align_y(Vertical::Center);

        container(body).width(Length::Fixed(width)).height(Length::Fixed(ROW_HEIGHT)).into()
    }
}

fn marker<'a>(expanded: bool) -> Element<'a, Message> {
    let glyph = text(if expanded { FOLDER_OPEN } else { FOLDER_SHUT })
        .font(fonts::MISC_SYMBOLS)
        .size(MARKER_SIZE)
        .line_height(MARKER_LINE_HEIGHT)
        .width(Length::Fixed(MARKER_WIDTH));

    match expanded {
        true => glyph.style(open_mark).into(),
        false => glyph.into(),
    }
}
