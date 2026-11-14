use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, toggler, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use codex::common;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(520.0, 560.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Logins, SIZE);

pub(super) const TITLE: &str = "Login Event";
pub(super) const FOLDER: &str = "Login Events";
pub(super) const TABLE: &str = "DailyLoginEventData.csv";

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const ROW_GAP: f32 = 4.0;
const HEADING_SIZE: f32 = 14.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const LABEL_WIDTH: f32 = 130.0;
const NUMBER_WIDTH: f32 = 80.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This event's row could not be read";
const EVENT_HEADING: &str = "Event";
const HEAD_LABELS: [&str; HEAD] = ["Event ID", "Title Image", "Text Red", "Text Green", "Text Blue", "Background Image", "Restarts Each Time"];
const DAY_LABEL: &str = "Stamp";
const ICON_LABEL: &str = "Shown Item ID";
const REWARD_LABELS: [&str; 3] = ["Category", "ID", "Amount"];
const ADD_REWARD: &str = "Add Reward";
const ADD_DAY: &str = "Add Stamp";
const REMOVE_DAY: &str = "Remove Stamp";
const REMOVE_LABEL: &str = "Remove";
const NOTE: &str = "Each stamp shows one item and gives every reward listed under it\nA background of -1 uses none\nAn event that restarts begins from its first stamp every time it runs";
const DELIMITER: char = ',';

const HEAD: usize = 7;
const EVENT_ID: usize = 0;
const REPEATS: usize = 6;
const REWARD_CELLS: usize = 3;
const FRESH_REWARD: [&str; REWARD_CELLS] = ["0", "0", "1"];

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    line: usize,
}

pub(super) fn plan(label: String, file: String, game: PathBuf, target_mod: String, values: EditorMode, line: usize) -> Plan {
    Plan { label, file, game, target_mod, values, line }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

fn numeric(typed: &str) -> bool {
    let digits = typed.strip_prefix('-').unwrap_or(typed);

    !digits.is_empty() && digits.chars().all(|glyph| glyph.is_ascii_digit())
}

#[derive(Clone, PartialEq, Debug)]
struct Day {
    icon: String,
    rewards: Vec<[String; REWARD_CELLS]>,
}

#[derive(Clone, PartialEq, Debug)]
struct Event {
    head: Vec<String>,
    days: Vec<Day>,
    tail: Vec<String>,
}

impl Event {
    fn parse(line: &str) -> Option<Event> {
        let cells: Vec<&str> = line.split(DELIMITER).collect();

        if cells.len() < HEAD || cells.first().is_none_or(|cell| !numeric(cell.trim())) {
            return None;
        }

        let head = cells[..HEAD].iter().map(|cell| (*cell).to_owned()).collect();
        let mut days = Vec::new();
        let mut column = HEAD;

        while column + 1 < cells.len() {
            let Some(count) = cells[column].trim().parse::<usize>().ok().filter(|count| column + 2 + count * REWARD_CELLS <= cells.len()) else {
                break;
            };
            let rewards = (0..count)
                .map(|at| {
                    let base = column + 2 + at * REWARD_CELLS;

                    [cells[base].to_owned(), cells[base + 1].to_owned(), cells[base + 2].to_owned()]
                })
                .collect();

            days.push(Day { icon: cells[column + 1].to_owned(), rewards });
            column += 2 + count * REWARD_CELLS;
        }

        Some(Event { head, days, tail: cells[column..].iter().map(|cell| (*cell).to_owned()).collect() })
    }

    fn render(&self) -> String {
        let mut cells: Vec<String> = self.head.clone();

        for day in &self.days {
            cells.push(day.rewards.len().to_string());
            cells.push(day.icon.clone());
            cells.extend(day.rewards.iter().flatten().cloned());
        }

        cells.extend(self.tail.iter().cloned());
        cells.join(&DELIMITER.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Head(usize),
    Icon(usize),
    Reward(usize, usize, usize),
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    event: Option<Event>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Login event editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let event = lines.get(plan.line).and_then(|line| Event::parse(line));

        Some(Draft { plan, read_from, stamp, lines, event, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn slot(&mut self, cell: Cell) -> Option<&mut String> {
        let event = self.event.as_mut()?;

        match cell {
            Cell::Head(at) => event.head.get_mut(at).filter(|_| at != EVENT_ID),
            Cell::Icon(day) => event.days.get_mut(day).map(|held| &mut held.icon),
            Cell::Reward(day, reward, at) => event.days.get_mut(day)?.rewards.get_mut(reward)?.get_mut(at),
        }
    }

    fn write(&mut self, cell: Cell, typed: &str) -> bool {
        let Some(slot) = self.slot(cell) else {
            return false;
        };

        if slot.trim() == typed {
            return false;
        }

        *slot = typed.to_owned();

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Login event editor could not read the vanilla file");

            return false;
        };

        self.lines = common::scrub(&bytes).lines().map(str::to_owned).collect();
        self.event = self.lines.get(self.plan.line).and_then(|line| Event::parse(line));

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Login event editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Login event editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        if let Some((event, slot)) = self.event.as_ref().zip(self.lines.get_mut(self.plan.line)) {
            *slot = event.render();
        }

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
                warn!(path = %path.display(), "Login event editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Intent {
    Day(usize),
    Reward(usize, usize),
    Sync,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(Cell, String),
    Repeats(bool),
    AddDay,
    AddReward(usize),
    RemoveDay(usize),
    RemoveReward(usize, usize),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<(Cell, String)>,
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

    fn reshape(&mut self, vfs: &Vfs, change: impl FnOnce(&mut Event) -> bool) -> Task<Message> {
        self.typed = None;

        let changed = self.draft.as_mut().and_then(|draft| draft.event.as_mut()).is_some_and(change);

        self.settle(changed, vfs)
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
            Message::Typed(cell, typed) => {
                let changed = numeric(&typed) && self.draft.as_mut().is_some_and(|draft| draft.write(cell, &typed));

                self.typed = Some((cell, typed));

                self.settle(changed, vfs)
            }
            Message::Repeats(on) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.write(Cell::Head(REPEATS), if on { "1" } else { "0" }));

                self.settle(changed, vfs)
            }
            Message::AddDay => self.reshape(vfs, |event| {
                let icon = event.days.last().map_or_else(|| FRESH_REWARD[1].to_owned(), |day| day.icon.clone());

                event.days.push(Day { icon, rewards: vec![FRESH_REWARD.map(str::to_owned)] });

                true
            }),
            Message::AddReward(day) => self.reshape(vfs, |event| {
                event.days.get_mut(day).is_some_and(|held| {
                    held.rewards.push(FRESH_REWARD.map(str::to_owned));

                    true
                })
            }),
            Message::RemoveDay(day) => {
                if !self.confirm.take(&Intent::Day(day)) {
                    return self.confirm.set(Intent::Day(day), Message::ConfirmExpired);
                }

                self.reshape(vfs, |event| {
                    if day >= event.days.len() {
                        return false;
                    }

                    event.days.remove(day);

                    true
                })
            }
            Message::RemoveReward(day, reward) => {
                if !self.confirm.take(&Intent::Reward(day, reward)) {
                    return self.confirm.set(Intent::Reward(day, reward), Message::ConfirmExpired);
                }

                self.reshape(vfs, |event| {
                    event.days.get_mut(day).filter(|held| reward < held.rewards.len()).is_some_and(|held| {
                        held.rewards.remove(reward);

                        true
                    })
                })
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

    fn number<'a>(&'a self, cell: Cell, current: &'a str) -> Element<'a, Message> {
        let value = match &self.typed {
            Some((held, typed)) if *held == cell => typed.as_str(),
            _ => current.trim(),
        };

        text_input("", value)
            .on_input(move |typed| Message::Typed(cell, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fixed(NUMBER_WIDTH))
            .style(theme::rounded_input)
            .into()
    }

    fn listing<'a>(&'a self, draft: &'a Draft, event: &'a Event) -> Element<'a, Message> {
        let raw = draft.plan.values == EditorMode::Raw;
        let mut cards = Column::new().spacing(CARD_GAP).width(Length::Fill);
        let mut head = Column::new().spacing(ROW_GAP).push(text(EVENT_HEADING).size(HEADING_SIZE));

        for (at, (label, held)) in HEAD_LABELS.iter().zip(&event.head).enumerate() {
            let field: Element<'_, Message> = match (at, raw) {
                (EVENT_ID, _) => text(held.trim()).size(INPUT_SIZE).into(),
                (REPEATS, false) => toggler(held.trim() != "0").on_toggle(Message::Repeats).style(theme::ios_toggle).into(),
                _ => self.number(Cell::Head(at), held),
            };

            head = head.push(row![text(*label).size(LABEL_SIZE).width(Length::Fixed(LABEL_WIDTH)), field].spacing(GAP).align_y(Vertical::Center));
        }

        cards = cards.push(card(head.into()));

        for (day, held) in event.days.iter().enumerate() {
            let mut rows = Column::new().spacing(ROW_GAP).push(text(format!("{DAY_LABEL} {}", day + 1)).size(HEADING_SIZE)).push(
                row![text(ICON_LABEL).size(LABEL_SIZE).width(Length::Fixed(LABEL_WIDTH)), self.number(Cell::Icon(day), &held.icon)]
                    .spacing(GAP)
                    .align_y(Vertical::Center),
            );

            for (reward, cells) in held.rewards.iter().enumerate() {
                let mut line = row![].spacing(GAP).align_y(Vertical::Center);

                for (at, (label, value)) in REWARD_LABELS.iter().zip(cells).enumerate() {
                    line = line.push(text(*label).size(LABEL_SIZE)).push(self.number(Cell::Reward(day, reward, at), value));
                }

                let armed = self.confirm.armed_for(&Intent::Reward(day, reward));
                let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                    .padding(SMALL_PADDING)
                    .style(theme::danger_button)
                    .on_press(Message::RemoveReward(day, reward));

                rows = rows.push(line.push(remove));
            }

            let armed = self.confirm.armed_for(&Intent::Day(day));
            let tools = row![
                button(theme::centered_text(ADD_REWARD).size(LABEL_SIZE)).padding(SMALL_PADDING).style(theme::primary_button).on_press(Message::AddReward(day)),
                button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_DAY }).size(LABEL_SIZE))
                    .padding(SMALL_PADDING)
                    .style(theme::danger_button)
                    .on_press(Message::RemoveDay(day)),
            ]
            .spacing(GAP);

            cards = cards.push(card(rows.push(tools).into()));
        }

        let add = button(theme::centered_text(ADD_DAY).size(LABEL_SIZE)).padding(SMALL_PADDING).style(theme::primary_button).on_press(Message::AddDay);

        cards.push(add).push(text(NOTE).size(NOTE_SIZE).style(text::secondary)).into()
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listing: Element<'_, Message> = match &draft.event {
            Some(event) => self.listing(draft, event),
            None => container(theme::centered_text(MISSING_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
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

        column![framed, sync].spacing(GAP).padding(BODY_PADDING).width(Length::Fill).height(Length::Fill).into()
    }
}

fn card(body: Element<'_, Message>) -> Element<'_, Message> {
    container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let line = |text: Option<std::rc::Rc<str>>| text.and_then(|text| text.lines().nth(plan.line).map(str::to_owned));
    let ours = line(reader.text(&held));
    let theirs = line(reader.text(&plan.game));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::Event;

    const ROW: &str = "35059,903,255,0,0,10,0,1,6,0,6,5000,2,967,3,20,1,1,753,1,1,2,0,2,1,";

    #[test]
    fn an_event_row_reads_its_stamps_and_writes_back_unchanged() {
        let event = Event::parse(ROW).unwrap();

        assert_eq!(event.days.len(), 3);
        assert_eq!(event.days[1].icon, "967");
        assert_eq!(event.days[1].rewards.len(), 2);
        assert_eq!(event.tail, [""]);
        assert_eq!(event.render(), ROW);
    }

    #[test]
    fn adding_a_reward_updates_the_count_the_game_reads() {
        let mut event = Event::parse("900,900,90,130,120,-1,0,1,6,0,6,5000").unwrap();

        event.days[0].rewards.push(["1".to_owned(), "278".to_owned(), "1".to_owned()]);
        assert_eq!(event.render(), "900,900,90,130,120,-1,0,2,6,0,6,5000,1,278,1");
    }

    #[test]
    fn a_broken_tail_is_kept_as_it_was() {
        let event = Event::parse("1,2,3,4,5,6,7,9,6,0,6").unwrap();

        assert!(event.days.is_empty());
        assert_eq!(event.render(), "1,2,3,4,5,6,7,9,6,0,6");
        assert!(Event::parse("//note").is_none());
    }
}
