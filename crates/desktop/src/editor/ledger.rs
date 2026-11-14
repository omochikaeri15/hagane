use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, toggler, Column, Row};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::combat::Separator;
use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

pub(crate) use foundry::systems::editor::ledger::*;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(460.0, 540.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Ledger, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;
const RAW_COLUMNS: usize = 3;

const SYNC_LABEL: &str = "Sync With \"game\"";
const REMOVE_LABEL: &str = "Remove";
const GROWTH_NOTE: &str = "From the level the effect's previous bracket ends at up to this one, the effect goes from From to To";
const LOCK_NOTE: &str = "The Super CPU can't run this map\nRemoving the lock lets it\nThe message type picks which notice the game shows";
const EX_NOTE: &str = "Clearing the stage may open an EX stage\nEach group is a row of EX_lottery.csv with its own chance";
const EX_EMPTY: &str = "None";
const EX_GROUP_LABELS: [&str; EX_PAIRS] = ["Group 1", "Group 2", "Group 3"];
const EX_CHANCE_LABELS: [&str; EX_PAIRS] = ["Chance 1", "Chance 2", "Chance 3"];
const LIMIT_NOTE: &str = "The stage stays locked until the game's unlock condition with this code is met";
const CHECKPOINT_NOTE: &str = "Clearing the stage lets later runs of this map start from any stage up to the limit";
const DRAW_NOTE: &str = "Each stage in the range plays a layout picked from the layout range\nEqual-sized ranges pair stages with layouts in order";

const RARITY: usize = 3;
const DEPLOY: usize = 4;
const ROWS: usize = 5;
const MIN_COST: usize = 6;
const MAX_COST: usize = 7;
const GROUP: usize = 8;

const FORMATION_CROWN: usize = 1;
const FORMATION_STAGE: usize = 2;

const DRAW_FIRST_STAGE: usize = 1;
const DRAW_LAST_STAGE: usize = 2;
const DRAW_FIRST_LAYOUT: usize = 3;
const DRAW_LAST_LAYOUT: usize = 4;

const CHECKPOINT_STAGE: usize = 1;
const CHECKPOINT_LIMIT: usize = 2;

const LIMIT_STAGE: usize = 1;
const LIMIT_CONDITION: usize = 2;

const EX_RATE: usize = 0;
const EX_STAGE: usize = 2;
const EX_PAIRS: usize = 3;

const LOCK_MESSAGE: usize = 0;

const GROWTH_EFFECT: usize = 1;
const GROWTH_LEVEL: usize = 2;
const GROWTH_FROM: usize = 3;
const GROWTH_TO: usize = 4;
const GROWTH_EASING: usize = 5;

const RARITIES: [&str; 6] = ["Normal", "Special", "Rare", "Super Rare", "Uber Rare", "Legend Rare"];

const CROWN_LABELS: [&str; 4] = ["1 Crown", "2 Crowns", "3 Crowns", "4 Crowns"];

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    ledger: Ledger,
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    key: u32,
    stages: Vec<(u32, String)>,
    presets: Vec<String>,
    effects: Vec<(i32, String)>,
    values: EditorMode,
}

pub(super) struct Seed {
    pub(super) ledger: Ledger,
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) key: u32,
    pub(super) stages: Vec<(u32, String)>,
    pub(super) presets: Vec<String>,
    pub(super) effects: Vec<(i32, String)>,
    pub(super) values: EditorMode,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { ledger, label, file, game, target_mod, key, stages, presets, effects, values } = seed;

    Plan { ledger, label, file, game, target_mod, key, stages, presets, effects, values }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

fn keyed(line: &str, delimiter: char, column: usize) -> Option<u32> {
    let mut cells = line.split(delimiter).map(str::trim);
    let lead = cells.next()?;

    lead.parse::<i64>().ok()?;

    match column.checked_sub(1) {
        Some(rest) => cells.nth(rest)?.parse().ok(),
        None => lead.parse().ok(),
    }
}

fn owned(lines: &[String], key: u32, delimiter: char, column: usize) -> Vec<usize> {
    lines.iter().enumerate().filter(|(_, line)| keyed(line, delimiter, column) == Some(key)).map(|(at, _)| at).collect()
}

fn entry(plan: &Plan, body: &str) -> Vec<String> {
    let delimiter = Separator::detect(body).unwrap_or(Separator::Comma).char();

    let column = plan.ledger.key_column();

    body.lines().filter(|line| keyed(line, delimiter, column) == Some(plan.key)).map(str::to_owned).collect()
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    delimiter: char,
    lines: Vec<String>,
    rows: Vec<usize>,
    cells: Vec<Vec<String>>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Table editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let body = common::scrub(&bytes);
        let delimiter = Separator::detect(&body).unwrap_or(Separator::Comma).char();
        let lines: Vec<String> = body.lines().map(str::to_owned).collect();

        let mut draft = Draft { plan, read_from, stamp, delimiter, lines, rows: Vec::new(), cells: Vec::new(), failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        self.rows = owned(&self.lines, self.plan.key, self.delimiter, self.plan.ledger.key_column());

        let ledger = self.plan.ledger;
        let width = ledger.width();

        self.cells = self
            .rows
            .iter()
            .filter_map(|at| self.lines.get(*at))
            .map(|line| {
                let mut cells: Vec<String> = line.split(self.delimiter).map(str::to_owned).collect();

                while cells.len() < width {
                    cells.push(ledger.blank(cells.len()));
                }

                cells
            })
            .collect();
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn cell(&self, row: usize, column: usize) -> &str {
        self.cells.get(row).and_then(|cells| cells.get(column)).map_or("", String::as_str)
    }

    fn read(&self, row: usize, column: usize) -> i32 {
        self.cells
            .get(row)
            .and_then(|cells| cells.get(column))
            .and_then(|cell| cell.trim().parse().ok())
            .unwrap_or(0)
    }

    fn write(&mut self, row: usize, column: usize, value: String) -> bool {
        let Some(cell) = self.cells.get_mut(row).and_then(|cells| cells.get_mut(column)) else {
            return false;
        };

        if *cell == value {
            return false;
        }

        *cell = value;

        let trims = self.plan.ledger.trims();
        let delimiter = self.delimiter;

        let joined = self.cells.get(row).map(|cells| {
            let line = cells.join(&delimiter.to_string());

            match trims {
                true => line.trim_end_matches(delimiter).to_owned(),
                false => line,
            }
        });

        let (Some(line), Some(slot)) = (joined, self.rows.get(row).and_then(|at| self.lines.get_mut(*at))) else {
            return false;
        };

        *slot = line;

        true
    }

    fn add(&mut self) {
        let at = self.rows.last().map_or(self.lines.len(), |last| last + 1);
        let seeded = self.plan.ledger.seed(self.plan.key, &self.plan.presets).join(&self.delimiter.to_string());

        self.lines.insert(at, seeded);
        self.index();
    }

    fn remove(&mut self, row: usize) -> bool {
        let Some(at) = self.rows.get(row).copied() else {
            return false;
        };

        self.lines.remove(at);
        self.index();

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Table editor could not read the vanilla file");

            return false;
        };

        let vanilla = entry(&self.plan, &common::scrub(&bytes));
        let at = self.rows.first().copied().unwrap_or(self.lines.len());

        for line in self.rows.iter().rev() {
            self.lines.remove(*line);
        }

        let at = at.min(self.lines.len());

        for (offset, line) in vanilla.into_iter().enumerate() {
            self.lines.insert(at + offset, line);
        }

        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Table editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Table editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Table editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

fn typable(typed: &str) -> bool {
    let digits = typed.strip_prefix('-').unwrap_or(typed);

    digits.chars().all(|glyph| glyph.is_ascii_digit())
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Pick {
    raw: i32,
    label: String,
}

impl std::fmt::Display for Pick {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

fn fixed_crowns() -> Vec<Pick> {
    CROWN_LABELS.iter().enumerate().map(|(at, label)| Pick { raw: at as i32, label: (*label).to_owned() }).collect()
}

fn fixed_stages(plan: &Plan) -> Vec<Pick> {
    plan.stages.iter().map(|(stage, name)| Pick { raw: *stage as i32, label: name.clone() }).collect()
}

fn crowns() -> Vec<Pick> {
    let mut held = vec![Pick { raw: EVERY, label: "Every Crown".to_owned() }];

    held.extend(CROWN_LABELS.iter().enumerate().map(|(at, label)| Pick { raw: at as i32, label: (*label).to_owned() }));

    held
}

fn stages(plan: &Plan) -> Vec<Pick> {
    let mut held = vec![Pick { raw: EVERY, label: "Every Stage".to_owned() }];

    held.extend(plan.stages.iter().map(|(stage, name)| Pick { raw: *stage as i32, label: name.clone() }));

    held
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Intent {
    Remove(usize),
    Sync,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Changed(usize, usize, String),
    Chosen(usize, usize, i32),
    Named(usize, usize, String),
    Flipped(usize, u32),
    Rows(usize, bool),
    Added,
    Removed(usize),
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
            Message::Changed(row, column, typed) => {
                let worded = self.draft.as_ref().is_some_and(|draft| draft.plan.ledger.text(column));

                if worded {
                    self.typed = Some((row, column, typed.clone()));

                    let changed = self.draft.as_mut().is_some_and(|draft| draft.write(row, column, typed));

                    return self.settle(changed, vfs);
                }

                if !typable(&typed) {
                    return Task::none();
                }

                let parsed = typed.parse::<i32>().ok();
                self.typed = Some((row, column, typed));

                let Some(value) = parsed else {
                    return Task::none();
                };

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(row, column, value.to_string()));

                self.settle(changed, vfs)
            }
            Message::Named(row, column, name) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(row, column, name));

                self.settle(changed, vfs)
            }
            Message::Chosen(row, column, raw) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(row, column, raw.to_string()));

                self.settle(changed, vfs)
            }
            Message::Flipped(row, bit) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mask = draft.read(row, RARITY) ^ (1 << bit);

                    draft.write(row, RARITY, mask.to_string())
                });

                self.settle(changed, vfs)
            }
            Message::Rows(row, single) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(row, ROWS, i32::from(single).to_string()));

                self.settle(changed, vfs)
            }
            Message::Added => {
                self.typed = None;

                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                draft.add();

                self.settle(true, vfs)
            }
            Message::Removed(row) => {
                if !self.confirm.take(&Intent::Remove(row)) {
                    return self.confirm.set(Intent::Remove(row), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.remove(row));

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

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut cards = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for row in 0..draft.cells.len() {
            cards = cards.push(self.card(draft, row));
        }

        let listing: Element<'_, Message> = match draft.cells.is_empty() {
            true => container(theme::centered_text(draft.plan.ledger.empty_label()).size(LABEL_SIZE).style(text::secondary))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => smooth_scroll(
                scrollable(container(cards).padding(Padding::default().right(SCROLL_GAP)))
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .into(),
        };

        let framed = container(listing)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(FRAME_INSET)
            .style(theme::card_container_outlined);

        let add = button(theme::button_label(draft.plan.ledger.add_label()).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::primary_button)
            .on_press(Message::Added);

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

        column![framed, row![add, sync].spacing(GAP)]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn card<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        let armed = self.confirm.armed_for(&Intent::Remove(row));

        let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Removed(row));

        let heading = row![text(format!("{} {}", draft.plan.ledger.noun(), row + 1)).size(LABEL_SIZE).width(Length::Fill), remove]
            .align_y(Vertical::Center);

        let body = match (draft.plan.values, draft.plan.ledger) {
            (EditorMode::Raw, _) => self.raw(draft, row),
            (EditorMode::Resolved, Ledger::StageOption) => self.resolved(draft, row),
            (EditorMode::Resolved, Ledger::FixedFormation) => self.formation(draft, row),
            (EditorMode::Resolved, Ledger::StageDraws) => self.draws(draft, row),
            (EditorMode::Resolved, Ledger::Checkpoints) => self.checkpoint(draft, row),
            (EditorMode::Resolved, Ledger::StageLimits) => self.unlock(draft, row),
            (EditorMode::Resolved, Ledger::ExDraws) => self.ex_draw(draft, row),
            (EditorMode::Resolved, Ledger::CpuLocks) => self.lock(draft, row),
            (EditorMode::Resolved, Ledger::Growth(_)) => self.growth(draft, row),
        };

        container(column![heading, body].spacing(CARD_GAP))
            .width(Length::Fill)
            .padding(CARD_PADDING)
            .style(theme::card_container_outlined)
            .into()
    }

    fn input<'a>(&'a self, draft: &'a Draft, row: usize, column: usize) -> Element<'a, Message> {
        let shown = match &self.typed {
            Some((held_row, held_column, typed)) if *held_row == row && *held_column == column => typed.clone(),
            _ if draft.plan.ledger.text(column) => draft.cell(row, column).to_owned(),
            _ if draft.cell(row, column).trim().is_empty() => String::new(),
            _ => draft.read(row, column).to_string(),
        };

        text_input("", &shown)
            .on_input(move |typed| Message::Changed(row, column, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input)
            .into()
    }

    fn labeled<'a>(&'a self, label: &'a str, control: Element<'a, Message>) -> Element<'a, Message> {
        column![text(label).size(LABEL_SIZE), control].spacing(2).width(Length::Fill).into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        let ledger = draft.plan.ledger;
        let mut grid = Column::new().spacing(CARD_GAP);

        for start in (0..ledger.width()).step_by(RAW_COLUMNS) {
            let mut line = Row::new().spacing(GAP);

            for column in start..(start + RAW_COLUMNS).min(ledger.width()) {
                line = line.push(self.labeled(ledger.field(column), self.input(draft, row, column)));
            }

            grid = grid.push(line);
        }

        grid.into()
    }

    fn stage_pick<'a>(&'a self, draft: &'a Draft, row: usize, column: usize) -> Element<'a, Message> {
        let options = fixed_stages(&draft.plan);
        let current = draft.read(row, column);
        let chosen = options.iter().find(|pick| pick.raw == current).cloned();

        pick_list(options, chosen, move |pick: Pick| Message::Chosen(row, column, pick.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box)
            .into()
    }

    fn growth<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        let options: Vec<Pick> = draft.plan.effects.iter().map(|(raw, label)| Pick { raw: *raw, label: label.clone() }).collect();
        let current = draft.read(row, GROWTH_EFFECT);
        let chosen = options.iter().find(|pick| pick.raw == current).cloned();

        let effect: Element<'_, Message> = match options.is_empty() {
            true => self.input(draft, row, GROWTH_EFFECT),
            false => pick_list(options, chosen, move |pick: Pick| Message::Chosen(row, GROWTH_EFFECT, pick.raw))
                .placeholder(current.to_string())
                .text_size(INPUT_SIZE)
                .width(Length::Fill)
                .style(theme::combo_box)
                .into(),
        };

        column![
            row![self.labeled("Effect", effect), self.labeled("Up To Level", self.input(draft, row, GROWTH_LEVEL))].spacing(GAP),
            row![
                self.labeled("From", self.input(draft, row, GROWTH_FROM)),
                self.labeled("To", self.input(draft, row, GROWTH_TO)),
                self.labeled("Easing", self.input(draft, row, GROWTH_EASING)),
            ]
            .spacing(GAP),
            text(GROWTH_NOTE).size(LABEL_SIZE - 1.0).style(text::secondary),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn lock<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        column![
            self.labeled("Message Type", self.input(draft, row, LOCK_MESSAGE)),
            text(LOCK_NOTE).size(LABEL_SIZE - 1.0).style(text::secondary),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn ex_draw<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        let mut body = column![
            row![self.labeled("Chance %", self.input(draft, row, EX_RATE)), self.labeled("After Stage", self.stage_pick(draft, row, EX_STAGE))]
                .spacing(GAP),
        ]
        .spacing(CARD_GAP);

        for pair in 0..EX_PAIRS {
            let group = EX_FIRST_PAIR + pair * 2;
            let options: Vec<Pick> = draft.plan.effects.iter().map(|(raw, label)| Pick { raw: *raw, label: label.clone() }).collect();
            let held = draft.cell(row, group).trim().parse::<i32>().ok();
            let chosen = options.iter().find(|pick| Some(pick.raw) == held).cloned();

            let pick = pick_list(options, chosen, move |pick: Pick| Message::Chosen(row, group, pick.raw))
                .placeholder(EX_EMPTY)
                .text_size(INPUT_SIZE)
                .width(Length::Fill)
                .style(theme::combo_box);

            body = body.push(
                row![
                    self.labeled(EX_GROUP_LABELS[pair], pick.into()),
                    self.labeled(EX_CHANCE_LABELS[pair], self.input(draft, row, group + 1)),
                ]
                .spacing(GAP),
            );
        }

        body.push(text(EX_NOTE).size(LABEL_SIZE - 1.0).style(text::secondary)).into()
    }

    fn unlock<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        column![
            row![
                self.labeled("Stage", self.stage_pick(draft, row, LIMIT_STAGE)),
                self.labeled("Unlock Condition Code", self.input(draft, row, LIMIT_CONDITION)),
            ]
            .spacing(GAP),
            text(LIMIT_NOTE).size(LABEL_SIZE - 1.0).style(text::secondary),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn checkpoint<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        column![
            row![
                self.labeled("Cleared Stage", self.stage_pick(draft, row, CHECKPOINT_STAGE)),
                self.labeled("Skip Up To", self.stage_pick(draft, row, CHECKPOINT_LIMIT)),
            ]
            .spacing(GAP),
            self.labeled("Message Text Key", self.input(draft, row, CHECKPOINT_MESSAGE)),
            text(CHECKPOINT_NOTE).size(LABEL_SIZE - 1.0).style(text::secondary),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn draws<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        column![
            row![
                self.labeled("From Stage", self.stage_pick(draft, row, DRAW_FIRST_STAGE)),
                self.labeled("To Stage", self.stage_pick(draft, row, DRAW_LAST_STAGE)),
            ]
            .spacing(GAP),
            row![
                self.labeled("First Layout ID", self.input(draft, row, DRAW_FIRST_LAYOUT)),
                self.labeled("Last Layout ID", self.input(draft, row, DRAW_LAST_LAYOUT)),
            ]
            .spacing(GAP),
            text(DRAW_NOTE).size(LABEL_SIZE - 1.0).style(text::secondary),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn formation<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        let crown_options = fixed_crowns();
        let crown = draft.read(row, FORMATION_CROWN);
        let chosen_crown = crown_options.iter().find(|pick| pick.raw == crown).cloned();

        let crown_pick = pick_list(crown_options, chosen_crown, move |pick: Pick| Message::Chosen(row, FORMATION_CROWN, pick.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let stage_options = fixed_stages(&draft.plan);
        let stage = draft.read(row, FORMATION_STAGE);
        let chosen_stage = stage_options.iter().find(|pick| pick.raw == stage).cloned();

        let stage_pick = pick_list(stage_options, chosen_stage, move |pick: Pick| Message::Chosen(row, FORMATION_STAGE, pick.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let current = draft.cell(row, FORMATION_PRESET).trim().to_owned();
        let mut presets = draft.plan.presets.clone();

        if !current.is_empty() && !presets.contains(&current) {
            presets.push(current.clone());
        }

        let chosen_preset = presets.iter().find(|name| **name == current).cloned();

        let preset_pick = pick_list(presets, chosen_preset, move |name: String| Message::Named(row, FORMATION_PRESET, name))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        column![
            row![self.labeled("Crown", crown_pick.into()), self.labeled("Stage", stage_pick.into())].spacing(GAP),
            self.labeled("Lineup Preset", preset_pick.into()),
            self.labeled("Memo", self.input(draft, row, FORMATION_MEMO)),
        ]
        .spacing(CARD_GAP)
        .into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft, row: usize) -> Element<'a, Message> {
        let crown_options = crowns();
        let crown = draft.read(row, CROWN);
        let chosen_crown = crown_options.iter().find(|pick| pick.raw == crown).cloned();

        let crown_pick = pick_list(crown_options, chosen_crown, move |pick: Pick| Message::Chosen(row, CROWN, pick.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let stage_options = stages(&draft.plan);
        let stage = draft.read(row, STAGE);
        let chosen_stage = stage_options.iter().find(|pick| pick.raw == stage).cloned();

        let stage_pick = pick_list(stage_options, chosen_stage, move |pick: Pick| Message::Chosen(row, STAGE, pick.raw))
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let mask = draft.read(row, RARITY);

        let mut rarities = Row::new().spacing(4);

        for (bit, name) in RARITIES.iter().enumerate() {
            let held = mask & (1 << bit) != 0;
            let style: theme::ButtonStyleFn = if held { theme::success_button } else { theme::neutral_button };

            rarities = rarities.push(
                button(theme::centered_text(*name).size(LABEL_SIZE - 1.0))
                    .padding([3.0, 4.0])
                    .width(Length::Fill)
                    .style(style)
                    .on_press(Message::Flipped(row, bit as u32)),
            );
        }

        let rarity_label = if mask == 0 { "Rarities (any)" } else { "Rarities" };
        let single = draft.read(row, ROWS) == 1;

        let rows_toggle = toggler(single)
            .label("Front row only")
            .text_size(LABEL_SIZE)
            .on_toggle(move |value| Message::Rows(row, value))
            .style(theme::ios_toggle);

        column![
            row![self.labeled("Crown", crown_pick.into()), self.labeled("Stage", stage_pick.into())].spacing(GAP),
            self.labeled(rarity_label, rarities.into()),
            row![self.labeled("Deploy Limit (0 = none)", self.input(draft, row, DEPLOY)), container(rows_toggle).width(Length::Fill).padding(Padding::default().top(14.0))]
                .spacing(GAP),
            row![
                self.labeled("Min Cost", self.input(draft, row, MIN_COST)),
                self.labeled("Max Cost", self.input(draft, row, MAX_COST)),
                self.labeled("Group ID", self.input(draft, row, GROUP)),
            ]
            .spacing(GAP),
        ]
        .spacing(CARD_GAP)
        .into()
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let rows = |path: &Path, reader: &mut Reader| reader.text(path).map(|body| entry(plan, &body)).filter(|rows| !rows.is_empty());

    let ours = rows(&held, reader);
    let theirs = rows(&plan.game, reader);

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{owned, Ledger};

    #[test]
    fn a_map_owns_only_the_rows_keyed_to_it() {
        let lines: Vec<String> =
            ["//mapID,crown", "3006,0,1,61", "3007,0,1,0", "3006,-1,-1,0", "x,1"].iter().map(|line| (*line).to_owned()).collect();

        assert_eq!(owned(&lines, 3006, ',', 0), vec![1, 3]);

        // EX_group keys a row by its second cell, and its comment lines never count
        let grouped: Vec<String> = ["100,24020,0,0,1", "100,24021,0,0,1", "//24020"].iter().map(|line| (*line).to_owned()).collect();
        assert_eq!(owned(&grouped, 24020, ',', 1), vec![0]);
    }

    #[test]
    fn a_new_restriction_covers_every_crown_and_stage_and_keeps_the_trailing_cell() {
        assert_eq!(Ledger::StageOption.seed(3006, &[]).join(","), "3006,-1,-1,0,0,0,0,0,0,");
    }
}
