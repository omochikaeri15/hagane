use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, mouse_area, operation, responsive, row, scrollable, space, stack, text, text_input, Column};
use iced::{mouse, widget, Border, Color, Element, Font, Length, Padding, Point, Size, Task, Theme};

use foundry::domains::sandbox::replay::{self as tape, ShelfRow};

use crate::app::theme;
use crate::common::row_window::{self, RowWindow};
use crate::common::{fonts, glyphs};
use crate::systems::animation::controls;
use crate::widget::{branches, list_row, open_mark, smooth_scroll, Branches, Guide, Tracer};

use super::{LATEST_LABEL, LIST_GAP, LIST_PADDING, LIST_WIDTH, Message, Pick, SEARCH_GAP, State};

const TEXT_SIZE: f32 = 13.0;
const CHAR_WIDTH: f32 = TEXT_SIZE * 0.65;
const ROW_HEIGHT: f32 = 24.0;
const ROW_PADDING: f32 = 6.0;
const INDENT: f32 = 12.0;
const MARKER_SIZE: f32 = ROW_HEIGHT * fonts::TRIANGLE_ROW;
const MARKER_LINE_HEIGHT: f32 = ROW_HEIGHT / MARKER_SIZE;
const MARKER_WIDTH: f32 = 16.0;
const SCROLL_TAIL: f32 = 14.0;
const SCROLLBAR_ALLOWANCE: f32 = 14.0;
const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 2.0;
const FOLDER_OPEN: &str = "\u{25be}";
const FOLDER_SHUT: &str = "\u{25b8}";
const BUTTON_GAP: f32 = 4.0;
const BUTTON_TEXT_SIZE: f32 = 12.0;
const BUTTON_PADDING: [f32; 2] = [8.0, 6.0];
const BUTTON_WIDTH: f32 = (LIST_WIDTH - BUTTON_GAP) / 2.0;
const HEADER_HEIGHT: f32 = 32.0;
const FIELD_HEIGHT: f32 = 28.0;
const TREE_TOP: f32 = LIST_PADDING + HEADER_HEIGHT + SEARCH_GAP + FIELD_HEIGHT + SEARCH_GAP + FIELD_HEIGHT + LIST_GAP;
const PANEL_WIDTH: f32 = LIST_WIDTH + LIST_PADDING * 2.0;
const DRAG_HASTE: f32 = 2.0;
const EDGE_BAND: f32 = 24.0;
const EDGE_STEP: f32 = ROW_HEIGHT;
const CARRIED_TINT: f32 = 0.25;
const NEST_BORDER: f32 = 1.5;
const GHOST_INK: f32 = 0.85;
const CREATE_LABEL: &str = "Create Folder";
const DELETE_LABEL: &str = "Delete Folder";
const CONFIRM_LABEL: &str = "Continue?";
const EMPTY_LABEL: &str = "No Replays Found";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Landing {
    Root,
    Row(usize),
}

#[derive(Clone, Debug, Default)]
pub(super) enum Drag {
    #[default]
    Idle,
    Pressed {
        row: usize,
        since: Instant,
    },
    Moving {
        item: PathBuf,
        name: String,
        at: Point,
        onto: Option<Landing>,
    },
}

impl Drag {
    fn ripe(&self) -> bool {
        match self {
            Drag::Pressed { since, .. } => since.elapsed() >= Duration::from_secs_f32(controls::HOLD_DELAY_SECS / DRAG_HASTE),
            _ => false,
        }
    }
}

pub(super) struct Row {
    entry: ShelfRow,
    guide: Guide,
}

fn tree_id() -> widget::Id {
    widget::Id::new("replay-tree")
}

pub(super) fn rename_id() -> widget::Id {
    widget::Id::new("replay-rename")
}

fn both_ways() -> scrollable::Direction {
    let bar = || scrollable::Scrollbar::new().width(SCROLLBAR_WIDTH).margin(SCROLLBAR_MARGIN);

    scrollable::Direction::Both { vertical: bar(), horizontal: bar() }
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

fn folder_button<'a>(label: &'a str, style: theme::ButtonStyleFn) -> button::Button<'a, Message> {
    button(theme::centered_text(label).size(BUTTON_TEXT_SIZE).wrapping(text::Wrapping::None)).width(Length::Fixed(BUTTON_WIDTH)).padding(BUTTON_PADDING).style(style)
}

fn flush(entry: &ShelfRow) -> bool {
    !entry.folder && entry.depth == 0
}

fn seat(theme: &Theme, carried: bool, nested: bool) -> container::Style {
    let palette = theme.palette();

    container::Style {
        background: Some(if carried { Color { a: CARRIED_TINT, ..palette.primary } } else { Color::TRANSPARENT }.into()),
        border: if nested { Border::default().rounded(4.0).width(NEST_BORDER).color(palette.success) } else { Border::default().rounded(4.0).width(0.0) },
        ..container::Style::default()
    }
}

fn console_landing(theme: &Theme) -> container::Style {
    container::Style { border: Border::default().rounded(4.0).width(NEST_BORDER).color(theme.palette().success), ..theme::mock_console_container(theme) }
}

impl State {
    pub(super) fn reshelve(&mut self) {
        self.shelf = tape::Shelf::read(&tape::library());
        self.rebuild_rows();
    }

    pub(super) fn rebuild_rows(&mut self) {
        let entries = self.shelf.rows(&self.expanded, &self.search);
        let mut tracer = Tracer::default();
        let mut guides = vec![Guide::default(); entries.len()];

        for index in (0..entries.len()).rev() {
            let above = index.checked_sub(1).map(|previous| entries[previous].depth);

            guides[index] = tracer.back(entries[index].depth, above);
        }

        self.widest = entries
            .iter()
            .map(|entry| ROW_PADDING * 2.0 + if flush(entry) { 0.0 } else { MARKER_WIDTH } + INDENT * f32::from(entry.depth) + CHAR_WIDTH * glyphs::columns(&entry.name))
            .fold(0.0, f32::max);
        self.rows = entries.into_iter().zip(guides).map(|(entry, guide)| Row { entry, guide }).collect();
    }

    pub(super) fn rebase(&mut self, old: &Path, fresh: &Path) {
        self.expanded = self.expanded.drain().map(|path| tape::rebased(&path, old, fresh).unwrap_or(path)).collect();

        for (pick, _, _) in &mut self.cache {
            if let Pick::Bundle(path) = pick
                && let Some(moved) = tape::rebased(path, old, fresh)
            {
                *path = moved;
            }
        }

        if let Some(Pick::Bundle(path)) = &mut self.picked
            && let Some(moved) = tape::rebased(path, old, fresh)
        {
            *path = moved;
        }

        if let Some(path) = &mut self.folder
            && let Some(moved) = tape::rebased(path, old, fresh)
        {
            *path = moved;
        }
    }

    pub(super) fn select_folder(&mut self, folder: PathBuf) {
        let name = folder.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let count = self.shelf.count(&folder);

        self.rename.clone_from(&name);
        self.details = Some(super::Details {
            title: name,
            meta: vec![("Replays", count.to_string())],
            game: Vec::new(),
            stage: None,
            version: None,
            ready: false,
        });
        self.picked = None;
        self.folder = Some(folder);
        self.open = None;
        self.failure = None;
        self.staged = None;
        self.tiles.clear();
        self.icons.clear();
        self.coin = None;
        self.folder_confirm.clear();
    }

    pub(super) fn recount_folder(&mut self, folder: &Path) {
        let count = self.shelf.count(folder);

        if let Some(details) = &mut self.details {
            details.meta = vec![("Replays", count.to_string())];
        }
    }

    pub(super) fn press_row(&mut self, row: usize) {
        self.drag = Drag::Pressed { row, since: Instant::now() };
    }

    pub(super) fn move_row(&mut self, at: Point) -> Task<Message> {
        let ripe = self.drag.ripe();
        let carried = match &mut self.drag {
            Drag::Moving { item, name, .. } => Some((std::mem::take(item), std::mem::take(name))),
            Drag::Pressed { row, .. } if ripe => self.rows.get(*row).map(|row| (row.entry.path.clone(), row.entry.name.clone())),
            _ => None,
        };

        let Some((item, name)) = carried else {
            return Task::none();
        };

        let onto = self.landing(&item, at);

        self.drag = Drag::Moving { item, name, at, onto };
        self.edge_scroll(at)
    }

    fn edge_scroll(&mut self, at: Point) -> Task<Message> {
        let within = at.y - TREE_TOP;
        let height = self.tree_height.get();
        let reach = (self.rows.len() as f32 * ROW_HEIGHT - height).max(0.0);
        let step = match within {
            _ if within < EDGE_BAND => -EDGE_STEP,
            _ if within > height - EDGE_BAND => EDGE_STEP,
            _ => return Task::none(),
        };
        let offset = (self.tree_offset + step).clamp(0.0, reach);

        if offset == self.tree_offset {
            return Task::none();
        }

        self.tree_offset = offset;

        operation::scroll_to(tree_id(), scrollable::AbsoluteOffset { x: 0.0, y: offset })
    }

    fn landing(&self, item: &Path, at: Point) -> Option<Landing> {
        if at.x < 0.0 || at.x > PANEL_WIDTH || at.y < TREE_TOP {
            return None;
        }

        let row = ((at.y - TREE_TOP + self.tree_offset) / ROW_HEIGHT).floor() as usize;
        let target = match self.rows.get(row) {
            Some(row_at) if row_at.entry.folder => Landing::Row(row),
            Some(row_at) => row_at.entry.parent.map_or(Landing::Root, Landing::Row),
            None => Landing::Root,
        };

        match target {
            Landing::Row(row) if self.rows.get(row).is_some_and(|row| row.entry.path.starts_with(item)) => None,
            target => Some(target),
        }
    }

    pub(super) fn take_drag(&mut self) -> Drag {
        std::mem::take(&mut self.drag)
    }

    pub(super) fn landing_folder(&self, onto: Landing) -> Option<PathBuf> {
        match onto {
            Landing::Root => Some(self.shelf.root().to_path_buf()),
            Landing::Row(row) => self.rows.get(row).map(|row| row.entry.path.clone()),
        }
    }

    pub(super) fn row_entry(&self, row: usize) -> Option<&ShelfRow> {
        self.rows.get(row).map(|row| &row.entry)
    }

    pub(super) fn creation_parent(&self) -> PathBuf {
        match (&self.folder, &self.picked) {
            (Some(folder), _) => folder.clone(),
            (None, Some(Pick::Bundle(bundle))) => bundle.parent().map_or_else(tape::library, Path::to_path_buf),
            _ => tape::library(),
        }
    }

    pub(super) fn snap_tree(&mut self) -> Task<Message> {
        self.tree_offset = 0.0;

        operation::scroll_to(tree_id(), scrollable::AbsoluteOffset { x: 0.0, y: 0.0 })
    }

    pub(super) fn view_list(&self) -> Element<'_, Message> {
        let delete_label = if self.folder_confirm.is_set() { CONFIRM_LABEL } else { DELETE_LABEL };
        let header = row![
            folder_button(CREATE_LABEL, theme::success_button).on_press(Message::CreateFolder),
            folder_button(delete_label, theme::danger_button).on_press_maybe(self.folder.is_some().then_some(Message::DeleteFolder)),
        ]
            .spacing(BUTTON_GAP)
            .height(Length::Fixed(HEADER_HEIGHT));
        let search = text_input("Search Replays...", &self.search)
            .on_input(Message::Search)
            .padding(4)
            .size(13)
            .width(Length::Fill)
            .style(theme::rounded_input);
        let latest_picked = self.picked == Some(Pick::Latest);
        let latest = button(theme::button_label(LATEST_LABEL).size(13))
            .on_press(Message::Select(Pick::Latest))
            .padding([4, 8])
            .width(Length::Fill)
            .style(move |theme: &Theme, status| theme::toggle_button(theme, status, latest_picked));
        let tree: Element<'_, Message> = if self.rows.is_empty() {
            container(theme::centered_text(EMPTY_LABEL).size(13).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else {
            responsive(move |size: Size| self.view_rows(size)).into()
        };
        let aimed_root = matches!(self.drag, Drag::Moving { onto: Some(Landing::Root), .. });
        let framed = container(container(tree).padding(theme::CONSOLE_BORDER_WIDTH))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |theme: &Theme| if aimed_root { console_landing(theme) } else { theme::mock_console_container(theme) });

        container(
            column![
                header,
                space().height(Length::Fixed(SEARCH_GAP)),
                container(search).height(Length::Fixed(FIELD_HEIGHT)),
                space().height(Length::Fixed(SEARCH_GAP)),
                container(latest).height(Length::Fixed(FIELD_HEIGHT)),
                space().height(Length::Fixed(LIST_GAP)),
                framed,
            ]
            .height(Length::Fill),
        )
            .width(Length::Fixed(PANEL_WIDTH))
            .height(Length::Fill)
            .padding(LIST_PADDING)
            .style(theme::list_panel_container)
            .into()
    }

    fn view_rows(&self, size: Size) -> Element<'_, Message> {
        self.tree_height.set(size.height);

        let tail = if self.widest > size.width { SCROLL_TAIL } else { 0.0 };
        let RowWindow { range, pad_before, pad_after } = row_window::compute_with(self.rows.len(), size.height - tail, self.tree_offset, ROW_HEIGHT, 0.0);
        let width = self.widest.max(size.width - SCROLLBAR_ALLOWANCE);
        let font = glyphs::mono();
        let mut list = Column::with_capacity(range.len() + 3);

        if pad_before > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_before)));
        }

        for index in range {
            if let Some(row_at) = self.rows.get(index) {
                list = list.push(self.view_row(index, row_at, width, font));
            }
        }

        if pad_after > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_after)));
        }

        if tail > 0.0 {
            list = list.push(space().height(Length::Fixed(tail)));
        }

        smooth_scroll(
            scrollable(list)
                .id(tree_id())
                .direction(both_ways())
                .on_scroll(|viewport| Message::TreeScrolled(viewport.absolute_offset().y))
                .width(Length::Fill)
                .height(Length::Fill),
        )
            .into()
    }

    fn view_row<'a>(&'a self, index: usize, row_at: &'a Row, width: f32, font: Font) -> Element<'a, Message> {
        let entry = &row_at.entry;
        let name = text(entry.name.as_str()).font(font).size(TEXT_SIZE).shaping(glyphs::shaping(&entry.name)).wrapping(text::Wrapping::None);
        let stem = branches(row_at.guide, entry.depth, INDENT, ROW_HEIGHT, MARKER_SIZE);
        let (stem, label) = match (entry.folder, entry.expanded, flush(entry)) {
            (true, true, _) => (Some(stem.opened()), row![marker(true), name]),
            (true, false, _) => (Some(stem), row![marker(false), name]),
            (false, _, true) => (None, row![name]),
            (false, _, false) => (Some(stem.reach(MARKER_WIDTH)), row![name]),
        };
        let span = stem.as_ref().map_or(0.0, Branches::span);
        let inset = stem.as_ref().map_or(0.0, Branches::inset);
        let carried = matches!(&self.drag, Drag::Moving { item, .. } if *item == entry.path);
        let nested = matches!(self.drag, Drag::Moving { onto: Some(Landing::Row(row)), .. } if row == index);
        let landed = container(label.align_y(Vertical::Center))
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT))
            .align_y(Vertical::Center)
            .padding(Padding::default().left(inset).right(ROW_PADDING))
            .style(move |theme: &Theme| seat(theme, carried, nested));
        let grip = mouse_area(landed).on_press(Message::RowPress(index));
        let selected = if entry.folder { self.folder.as_deref() == Some(entry.path.as_path()) } else { matches!(&self.picked, Some(Pick::Bundle(held)) if *held == entry.path) };
        let rest = Length::Fixed((width - ROW_PADDING - span).max(0.0));
        let mut seated = row![space().width(Length::Fixed(ROW_PADDING))].align_y(Vertical::Center);

        if let Some(stem) = stem {
            seated = seated.push(stem);
        }

        let seated = seated.push(list_row(grip, selected, false, rest, Message::RowRelease));

        container(seated).width(Length::Fixed(width)).height(Length::Fixed(ROW_HEIGHT)).into()
    }

    fn ghost(name: &str, at: Point) -> Element<'_, Message> {
        let carried = text(name).font(glyphs::mono()).size(TEXT_SIZE).shaping(glyphs::shaping(name)).wrapping(text::Wrapping::None).style(|theme: &Theme| text::Style {
            color: Some(Color { a: GHOST_INK, ..theme.palette().text }),
        });

        container(carried)
            .padding(Padding::default().left(at.x + ROW_PADDING).top(at.y + ROW_PADDING))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub(super) fn with_drag<'a>(&'a self, body: Element<'a, Message>) -> Element<'a, Message> {
        let mut layers = stack![body];
        let grabbing = matches!(self.drag, Drag::Moving { .. });

        if !matches!(self.drag, Drag::Idle) {
            let ghost: Element<'_, Message> = match &self.drag {
                Drag::Moving { name, at, .. } => Self::ghost(name, *at),
                _ => space().width(Length::Fill).height(Length::Fill).into(),
            };

            layers = layers.push(
                mouse_area(ghost)
                    .interaction(if grabbing { mouse::Interaction::Grabbing } else { mouse::Interaction::Grab })
                    .on_move(Message::RowMove)
                    .on_release(Message::RowRelease),
            );
        }

        layers.into()
    }
}
