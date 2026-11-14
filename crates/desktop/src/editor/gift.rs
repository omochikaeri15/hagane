use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

pub(crate) use foundry::systems::editor::gift::*;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 460.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Gift, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const QUANTITY_WIDTH: f32 = 70.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This rank is no longer in the file";
const RAW_LABEL: &str = "Cells";
const RAW_NOTE: &str = "The rank, then item and quantity pairs, ended by -1";
const EMPTY_LABEL: &str = "This rank gives nothing";
const ADD_LABEL: &str = "Add Reward";
const REMOVE_LABEL: &str = "Remove";
const QUANTITY_LABEL: &str = "Qty";
const FULL_NOTE: &str = "A rank can hold up to 10 rewards";
const LIST_JOINT: &str = ", ";

const MAX_REWARDS: usize = 10;
const NEW_ITEM: i64 = 0;
const NEW_QUANTITY: i64 = 1;

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    rank: u32,
    items: Vec<(i64, String)>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) rank: u32,
    pub(super) items: Vec<(i64, String)>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, rank, items } = seed;

    Plan { label, file, game, target_mod, values, rank, items }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

fn keyed(line: &str) -> Option<u32> {
    line.split(DELIMITER).next()?.trim().parse().ok()
}

fn entry(body: &str, rank: u32) -> Option<String> {
    body.lines().find(|line| keyed(line) == Some(rank)).map(str::to_owned)
}

fn render(rank: u32, rewards: &[(i64, i64)]) -> String {
    let mut cells = vec![rank.to_string()];

    for (item, quantity) in rewards.iter().take(MAX_REWARDS) {
        cells.push(item.to_string());
        cells.push(quantity.to_string());
    }

    cells.push(TERMINATOR.to_string());

    cells.join(&DELIMITER.to_string())
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    row: Option<usize>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Rank reward editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let row = lines.iter().position(|line| keyed(line) == Some(plan.rank));

        Some(Draft { plan, read_from, stamp, lines, row, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn line(&self) -> &str {
        self.row.and_then(|at| self.lines.get(at)).map_or("", String::as_str)
    }

    fn rewards(&self) -> Vec<(i64, i64)> {
        rewards(self.line())
    }

    fn put(&mut self, line: String) -> bool {
        let Some(slot) = self.row.and_then(|at| self.lines.get_mut(at)) else {
            return false;
        };

        if *slot == line {
            return false;
        }

        *slot = line;

        true
    }

    fn set_rewards(&mut self, rewards: &[(i64, i64)]) -> bool {
        let line = render(self.plan.rank, rewards);

        self.put(line)
    }

    fn set_raw(&mut self, typed: &str) -> bool {
        let parsed: Option<Vec<i64>> = typed.split(DELIMITER).map(str::trim).filter(|cell| !cell.is_empty()).map(|cell| cell.parse().ok()).collect();

        let Some(cells) = parsed.filter(|cells| cells.first() == Some(&i64::from(self.plan.rank))) else {
            return false;
        };

        let line: Vec<String> = cells.iter().map(i64::to_string).collect();

        self.put(line.join(&DELIMITER.to_string()))
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Rank reward editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.rank) else {
            warn!(path = %self.plan.game.display(), "Rank reward editor found no vanilla copy of this rank");

            return false;
        };

        self.put(original);

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Rank reward editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Rank reward editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Rank reward editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Choice {
    raw: i64,
    label: String,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
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
    Raw(String),
    Item(usize, i64),
    Quantity(usize, String),
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
    typed: Option<(Option<usize>, String)>,
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
            Message::Raw(typed) => {
                let changed = self.draft.as_mut().is_some_and(|draft| draft.set_raw(&typed));

                self.typed = Some((None, typed));

                self.settle(changed, vfs)
            }
            Message::Item(at, item) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut held = draft.rewards();

                    if let Some(reward) = held.get_mut(at) {
                        reward.0 = item;
                    }

                    draft.set_rewards(&held)
                });

                self.settle(changed, vfs)
            }
            Message::Quantity(at, typed) => {
                if !typed.chars().all(|glyph| glyph.is_ascii_digit()) {
                    return Task::none();
                }

                let parsed = typed.parse::<i64>().ok();
                self.typed = Some((Some(at), typed));

                let Some(quantity) = parsed.filter(|quantity| *quantity >= 1) else {
                    return Task::none();
                };

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut held = draft.rewards();

                    if let Some(reward) = held.get_mut(at) {
                        reward.1 = quantity;
                    }

                    draft.set_rewards(&held)
                });

                self.settle(changed, vfs)
            }
            Message::Add => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut held = draft.rewards();

                    if held.len() >= MAX_REWARDS {
                        return false;
                    }

                    held.push((NEW_ITEM, NEW_QUANTITY));

                    draft.set_rewards(&held)
                });

                self.settle(changed, vfs)
            }
            Message::Remove(at) => {
                if !self.confirm.take(&Intent::Remove(at)) {
                    return self.confirm.set(Intent::Remove(at), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut held = draft.rewards();

                    if at < held.len() {
                        held.remove(at);
                    }

                    draft.set_rewards(&held)
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

    fn typed_or(&self, slot: Option<usize>, current: String) -> String {
        match &self.typed {
            Some((held, typed)) if *held == slot => typed.clone(),
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

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let current: Vec<&str> = draft.line().split(DELIMITER).map(str::trim).collect();

        let field = text_input("", &self.typed_or(None, current.join(LIST_JOINT)))
            .on_input(Message::Raw)
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        column![text(RAW_LABEL).size(LABEL_SIZE), field, text(RAW_NOTE).size(NOTE_SIZE).style(text::secondary)].spacing(4).into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let held = draft.rewards();
        let mut rows = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for (at, (item, quantity)) in held.iter().enumerate() {
            let mut options: Vec<Choice> = draft.plan.items.iter().map(|(raw, label)| Choice { raw: *raw, label: label.clone() }).collect();

            if !options.iter().any(|choice| choice.raw == *item) {
                options.push(Choice { raw: *item, label: item.to_string() });
            }

            let chosen = options.iter().find(|choice| choice.raw == *item).cloned();

            let pick = pick_list(options, chosen, move |picked: Choice| Message::Item(at, picked.raw))
                .text_size(INPUT_SIZE)
                .width(Length::Fill)
                .style(theme::combo_box);

            let amount = text_input("", &self.typed_or(Some(at), quantity.to_string()))
                .on_input(move |typed| Message::Quantity(at, typed))
                .padding(INPUT_PADDING)
                .size(INPUT_SIZE)
                .width(Length::Fixed(QUANTITY_WIDTH))
                .style(theme::rounded_input);

            let armed = self.confirm.armed_for(&Intent::Remove(at));

            let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                .padding(SMALL_PADDING)
                .style(theme::danger_button)
                .on_press(Message::Remove(at));

            rows = rows.push(row![pick, text(QUANTITY_LABEL).size(LABEL_SIZE), amount, remove].spacing(GAP).align_y(Vertical::Center));
        }

        if held.is_empty() {
            rows = rows.push(text(EMPTY_LABEL).size(NOTE_SIZE).style(text::secondary));
        }

        let add = button(theme::centered_text(ADD_LABEL).size(LABEL_SIZE))
            .padding(SMALL_PADDING)
            .width(Length::Fill)
            .style(theme::primary_button)
            .on_press_maybe((held.len() < MAX_REWARDS).then_some(Message::Add));

        let mut body = column![rows, add].spacing(CARD_GAP);

        if held.len() >= MAX_REWARDS {
            body = body.push(text(FULL_NOTE).size(NOTE_SIZE).style(text::secondary));
        }

        container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| entry(&body, plan.rank));
    let theirs = reader.text(&plan.game).and_then(|body| entry(&body, plan.rank));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{render, rewards};

    #[test]
    fn a_rank_reads_its_pairs_up_to_the_terminator_and_writes_them_back() {
        assert_eq!(rewards("900,1200,5,-1"), vec![(1200, 5)]);
        assert_eq!(rewards("1000,21,1,22,3,-1,9"), vec![(21, 1), (22, 3)]);
        assert_eq!(render(900, &[(1200, 5)]), "900,1200,5,-1");
    }
}
