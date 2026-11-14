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

pub(crate) use foundry::systems::editor::filter::*;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(400.0, 460.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::ComboFilter, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This tab is past the end of the file";
const RAW_LABEL: &str = "Combo Effect IDs";
const EMPTY_LABEL: &str = "This tab lists no combo effects";
const ADD_HINT: &str = "Add a combo effect";
const REMOVE_LABEL: &str = "Remove";
const NOTE: &str = "The combo effects this tab of the combo list shows\nTab 0 is the tab that shows all of them";
const LIST_JOINT: &str = ", ";
const UNKNOWN_EFFECT: &str = "Effect";

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    line: usize,
    effects: Vec<(i64, String)>,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) line: usize,
    pub(super) effects: Vec<(i64, String)>,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, line, effects } = seed;

    Plan { label, file, game, target_mod, values, line, effects }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn name(&self, effect: i64) -> String {
        self.effects.iter().find(|(id, _)| *id == effect).map_or_else(|| format!("{UNKNOWN_EFFECT} {effect}"), |(_, name)| name.clone())
    }
}

fn render(effects: &[i64]) -> String {
    let cells: Vec<String> = effects.iter().map(i64::to_string).collect();

    cells.join(&DELIMITER.to_string())
}

fn entry(body: &str, line: usize) -> Option<String> {
    body.lines().nth(line).map(str::to_owned)
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Combo filter editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        Some(Draft { plan, read_from, stamp, lines, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn present(&self) -> bool {
        self.plan.line < self.lines.len()
    }

    fn effects(&self) -> Vec<i64> {
        self.lines.get(self.plan.line).map_or_else(Vec::new, |line| listed(line))
    }

    fn put(&mut self, effects: &[i64]) -> bool {
        let rendered = render(effects);

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
            warn!(path = %self.plan.game.display(), "Combo filter editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.line) else {
            warn!(path = %self.plan.game.display(), "Combo filter editor found no vanilla copy of this tab");

            return false;
        };

        if let Some(slot) = self.lines.get_mut(self.plan.line) {
            *slot = original;
        }

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Combo filter editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Combo filter editor staged a file the index would not take: {}", err);
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
                warn!(path = %path.display(), "Combo filter editor could not write the file: {}", err);
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
    Add(i64),
    Remove(usize),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<String>,
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
                let parsed: Option<Vec<i64>> = typed.split(',').map(str::trim).filter(|cell| !cell.is_empty()).map(|cell| cell.parse().ok()).collect();
                let changed = parsed.is_some_and(|effects| self.draft.as_mut().is_some_and(|draft| draft.put(&effects)));

                self.typed = Some(typed);

                self.settle(changed, vfs)
            }
            Message::Add(effect) => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut held = draft.effects();

                    if held.contains(&effect) {
                        return false;
                    }

                    held.push(effect);

                    draft.put(&held)
                });

                self.settle(changed, vfs)
            }
            Message::Remove(at) => {
                if !self.confirm.take(&Intent::Remove(at)) {
                    return self.confirm.set(Intent::Remove(at), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    let mut held = draft.effects();

                    if at < held.len() {
                        held.remove(at);
                    }

                    draft.put(&held)
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

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listing: Element<'_, Message> = match (draft.present(), draft.plan.values) {
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
        let current: Vec<String> = draft.effects().iter().map(i64::to_string).collect();
        let shown = self.typed.clone().unwrap_or_else(|| current.join(LIST_JOINT));

        let field = text_input("", &shown)
            .on_input(Message::Raw)
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        column![text(RAW_LABEL).size(LABEL_SIZE), field].spacing(4).into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let plan = &draft.plan;
        let held = draft.effects();
        let mut rows = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for (at, effect) in held.iter().enumerate() {
            let armed = self.confirm.armed_for(&Intent::Remove(at));

            let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                .padding(SMALL_PADDING)
                .style(theme::danger_button)
                .on_press(Message::Remove(at));

            rows = rows.push(row![text(plan.name(*effect)).size(LABEL_SIZE).width(Length::Fill), remove].spacing(GAP).align_y(Vertical::Center));
        }

        if held.is_empty() {
            rows = rows.push(text(EMPTY_LABEL).size(NOTE_SIZE).style(text::secondary));
        }

        let options: Vec<Choice> = plan
            .effects
            .iter()
            .filter(|(id, _)| !held.contains(id))
            .map(|(raw, label)| Choice { raw: *raw, label: label.clone() })
            .collect();

        let add = pick_list(options, None::<Choice>, |picked: Choice| Message::Add(picked.raw))
            .placeholder(ADD_HINT)
            .text_size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::combo_box);

        let body = column![rows, add, text(NOTE).size(NOTE_SIZE).style(text::secondary)].spacing(CARD_GAP);

        container(body).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined).into()
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
    use super::{listed, render};

    #[test]
    fn a_tab_round_trips_its_effect_ids() {
        assert_eq!(listed("3\t6\t7\t10"), vec![3, 6, 7, 10]);
        assert_eq!(render(&[3, 6, 7, 10]), "3\t6\t7\t10");
    }
}
