use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, toggler, Column};
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

const SIZE: Size = Size::new(440.0, 520.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Catfruit, SIZE);

pub(super) const TABLE: &str = "Matatabi.tsv";
pub(super) const TITLE: &str = "Catfruit";
pub(super) const FOLDER: &str = "Catfruit";

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
const RAW_WIDTH: f32 = 220.0;
const NUMBER_WIDTH: f32 = 90.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This item is no longer in the file";
const TEXT_HEADING: &str = "Text Keys";
const GROWTH_HEADING: &str = "Grows Into";
const SEED_LABEL: &str = "Seed";
const GROUP_LABEL: &str = "Group";
const SORT_LABEL: &str = "Sort Order";
const REQUIRE_LABEL: &str = "Seeds Needed";
const GUIDE_LABEL: &str = "Guide Text";
const GROWTH_LABEL: &str = "Growth Text";
const WEIGHT_LABEL: &str = "Weight";
const GROWTH_NOTE: &str = "A seed grows into one of these items, picked by weight\nSeeds Needed is how many it takes (-1 means it can't be grown)";
const EMPTY_GROWTH: &str = "Grows into nothing";
const ADD_LABEL: &str = "Add";
const ADD_HINT: &str = "Item ID";
const REMOVE_LABEL: &str = "Remove";
const COLUMN_LABEL: &str = "Column";
const UNKNOWN_ITEM: &str = "Item";
const DELIMITER: char = '\t';
const DEFAULT_WEIGHT: u32 = 1;

const KEY: usize = 0;
const SEED: usize = 1;
const GROUP: usize = 2;
const SORT: usize = 3;
const REQUIRE: usize = 4;
const GUIDE: usize = 5;
const GROWTH_TEXT: usize = 6;
const GROWTH: usize = 7;

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    key: u32,
    items: Vec<(u32, String)>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) key: u32,
    pub(super) items: Vec<(u32, String)>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, key, mut items } = seed;

    items.sort_unstable_by_key(|(id, _)| *id);

    Plan { label, file, game, target_mod, values, key, items }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn known(&self, id: u32) -> Option<&str> {
        self.items.binary_search_by_key(&id, |(held, _)| *held).ok().and_then(|at| self.items.get(at)).map(|(_, name)| name.as_str())
    }

    fn name(&self, id: u32) -> String {
        self.known(id).map_or_else(|| format!("{UNKNOWN_ITEM} {id}"), |name| format!("{name} ({id})"))
    }
}

pub(super) fn keyed(line: &str) -> Option<u32> {
    line.split(DELIMITER).next()?.trim().parse().ok()
}

fn entry(body: &str, key: u32) -> Option<String> {
    body.lines().find(|line| keyed(line) == Some(key)).map(str::to_owned)
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
    header: Vec<String>,
    row: Option<usize>,
    cells: Vec<String>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Catfruit editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let header = lines.first().filter(|line| keyed(line).is_none()).map_or_else(Vec::new, |line| line.split(DELIMITER).map(str::to_owned).collect());

        let mut draft = Draft { plan, read_from, stamp, lines, header, row: None, cells: Vec::new(), failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        self.row = self.lines.iter().position(|line| keyed(line) == Some(self.plan.key));
        self.cells = self.row.and_then(|at| self.lines.get(at)).map_or_else(Vec::new, |line| line.split(DELIMITER).map(str::to_owned).collect());
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn cell(&self, at: usize) -> &str {
        self.cells.get(at).map_or("", String::as_str)
    }

    fn growth(&self) -> Vec<(u32, u32)> {
        self.cells.get(GROWTH..).unwrap_or_default().chunks(2).filter_map(|pair| Some((pair.first()?.trim().parse().ok()?, pair.get(1)?.trim().parse().ok()?))).collect()
    }

    fn stage(&mut self) -> bool {
        let Some(at) = self.row else {
            return false;
        };

        let line = self.cells.join(&DELIMITER.to_string());

        let Some(slot) = self.lines.get_mut(at) else {
            return false;
        };

        if *slot == line {
            return false;
        }

        *slot = line;

        true
    }

    fn write(&mut self, at: usize, value: &str) -> bool {
        if self.row.is_none() || at == KEY || value.contains(DELIMITER) {
            return false;
        }

        while self.cells.len() <= at {
            self.cells.push(String::new());
        }

        if let Some(cell) = self.cells.get_mut(at) {
            value.clone_into(cell);
        }

        self.stage()
    }

    fn set_growth(&mut self, growth: &[(u32, u32)]) -> bool {
        if self.row.is_none() {
            return false;
        }

        while self.cells.len() < GROWTH {
            self.cells.push(String::new());
        }

        self.cells.truncate(GROWTH);
        self.cells.extend(growth.iter().flat_map(|(weight, item)| [weight.to_string(), item.to_string()]));

        self.stage()
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Catfruit editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.key) else {
            warn!(path = %self.plan.game.display(), "Catfruit editor found no vanilla copy of this item");

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
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Catfruit editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Catfruit editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Catfruit editor could not write the file: {}", err);
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
    Cell(usize),
    Weight(usize),
    Adding,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(Field, String),
    Seed(bool),
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

                let raw = draft.plan.values == EditorMode::Raw;

                let changed = match field {
                    Field::Cell(at) if raw || matches!(at, GUIDE | GROWTH_TEXT) => draft.write(at, &typed),
                    Field::Cell(at) => numeric(&typed) && draft.write(at, &typed),
                    Field::Weight(at) => {
                        let mut growth = draft.growth();

                        match (typed.parse::<u32>(), growth.get_mut(at)) {
                            (Ok(weight), Some(pair)) => {
                                pair.0 = weight;

                                draft.set_growth(&growth)
                            }
                            _ => false,
                        }
                    }
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
            Message::Seed(on) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(SEED, if on { "1" } else { "0" }));

                self.settle(changed, vfs)
            }
            Message::Add => {
                let Some(item) = self.adding() else {
                    return Task::none();
                };

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut growth = draft.growth();

                    growth.push((DEFAULT_WEIGHT, item));

                    draft.set_growth(&growth)
                });

                self.settle(changed, vfs)
            }
            Message::Remove(at) => {
                if !self.confirm.take(&Intent::Remove(at)) {
                    return self.confirm.set(Intent::Remove(at), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut growth = draft.growth();

                    if at < growth.len() {
                        growth.remove(at);
                    }

                    draft.set_growth(&growth)
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

        (draft.plan.known(id).is_some() && !draft.growth().iter().any(|(_, item)| *item == id)).then_some(id)
    }

    fn typed_or(&self, field: Field, current: &str) -> String {
        match &self.typed {
            Some((held, typed)) if *held == field => typed.clone(),
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

    fn input<'a>(&'a self, field: Field, current: &str, width: Length) -> Element<'a, Message> {
        text_input("", &self.typed_or(field, current))
            .on_input(move |typed| Message::Typed(field, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(width)
            .style(theme::rounded_input)
            .into()
    }

    fn line<'a>(label: impl Into<String>, control: Element<'a, Message>) -> Element<'a, Message> {
        row![text(label.into()).size(LABEL_SIZE).width(Length::Fill), control].spacing(GAP).align_y(Vertical::Center).into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut rows = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for at in 0..draft.cells.len().max(GROWTH) {
            let label = draft.header.get(at).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| format!("{COLUMN_LABEL} {}", at + 1));

            let control: Element<'_, Message> = match at {
                KEY => text(draft.cell(KEY).to_owned()).size(INPUT_SIZE).width(Length::Fixed(RAW_WIDTH)).into(),
                _ => self.input(Field::Cell(at), draft.cell(at), Length::Fixed(RAW_WIDTH)),
            };

            rows = rows.push(Self::line(label, control));
        }

        rows.into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let plan = &draft.plan;
        let seeded = draft.cell(SEED).trim() != "0" && !draft.cell(SEED).trim().is_empty();
        let number = |at: usize| self.input(Field::Cell(at), draft.cell(at), Length::Fixed(NUMBER_WIDTH));

        let seed = toggler(seeded).on_toggle(Message::Seed).style(theme::ios_toggle);

        let item = card(
            column![
                text(plan.name(plan.key)).size(HEADING_SIZE),
                Self::line(SEED_LABEL, seed.into()),
                Self::line(GROUP_LABEL, number(GROUP)),
                Self::line(SORT_LABEL, number(SORT)),
                Self::line(REQUIRE_LABEL, number(REQUIRE)),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        let texts = card(
            column![
                text(TEXT_HEADING).size(HEADING_SIZE),
                Self::line(GUIDE_LABEL, self.input(Field::Cell(GUIDE), draft.cell(GUIDE), Length::Fixed(RAW_WIDTH))),
                Self::line(GROWTH_LABEL, self.input(Field::Cell(GROWTH_TEXT), draft.cell(GROWTH_TEXT), Length::Fixed(RAW_WIDTH))),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        let mut outcomes = Column::new().spacing(UNIT_GAP).width(Length::Fill);
        let growth = draft.growth();

        for (at, (weight, target)) in growth.iter().enumerate() {
            let armed = self.confirm.armed_for(&Intent::Remove(at));

            let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                .padding(SMALL_PADDING)
                .style(theme::danger_button)
                .on_press(Message::Remove(at));

            outcomes = outcomes.push(
                row![
                    text(plan.name(*target)).size(LABEL_SIZE).width(Length::Fill),
                    text(WEIGHT_LABEL).size(NOTE_SIZE),
                    self.input(Field::Weight(at), &weight.to_string(), Length::Fixed(NUMBER_WIDTH)),
                    remove
                ]
                .spacing(GAP)
                .align_y(Vertical::Center),
            );
        }

        if growth.is_empty() {
            outcomes = outcomes.push(text(EMPTY_GROWTH).size(NOTE_SIZE).style(text::secondary));
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

        let grows = card(
            column![
                text(GROWTH_HEADING).size(HEADING_SIZE),
                outcomes,
                row![field, add].spacing(GAP).align_y(Vertical::Center),
                text(preview).size(NOTE_SIZE).style(text::secondary),
                text(GROWTH_NOTE).size(NOTE_SIZE).style(text::secondary),
            ]
            .spacing(CARD_GAP)
            .into(),
        );

        column![item, texts, grows].spacing(CARD_GAP).width(Length::Fill).into()
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
    fn a_catfruit_row_is_found_by_its_item_and_never_by_the_header() {
        let body = "gatyaID\tseed\tgroup\n30\t1\t0\t0\t5\tguide\ttext\t97\t35\t3\t40\n35\t0\t0\t100\t-1\tguide\n";

        assert_eq!(keyed("gatyaID\tseed"), None);
        assert_eq!(entry(body, 35).as_deref(), Some("35\t0\t0\t100\t-1\tguide"));
    }
}
