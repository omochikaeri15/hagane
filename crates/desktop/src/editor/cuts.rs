use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::systems::animation::authoring::{Imgcut, CUT_FIELDS, CUT_NAME_FIELD};
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, CONFIRM_SHORT_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(520.0, 540.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Cuts, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const ROW_GAP: f32 = 4.0;
const LABEL_SIZE: f32 = 12.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const INDEX_WIDTH: f32 = 34.0;
const NUMBER_WIDTH: f32 = 60.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SMALL_PADDING: [f32; 2] = [3.0, 8.0];
const REMOVE_WIDTH: f32 = 76.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const ADD_LABEL: &str = "Add Cut";
const REMOVE_LABEL: &str = "Remove";
const INDEX_LABEL: &str = "#";
const NOTE: &str = "Each cut is a rectangle of the sheet, and models use cuts by their number\nRemoving a cut renumbers every cut after it";

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
}

pub(super) fn plan(label: String, file: String, game: PathBuf, target_mod: String) -> Plan {
    Plan { label, file, game, target_mod }
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

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    doc: Imgcut,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Cut editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let doc = Imgcut::parse(&bytes)
            .inspect_err(|err| warn!(path = %read_from.display(), "Cut editor could not parse the file: {}", err))
            .ok()?;

        Some(Draft { plan, read_from, stamp, doc, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn write(&mut self, at: usize, cell: usize, typed: &str) -> bool {
        if cell == CUT_NAME_FIELD {
            return self.doc.set_name(at, typed);
        }

        typed.parse::<i32>().is_ok_and(|value| self.doc.set_field(at, cell, value))
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let parsed = fs::read(&self.plan.game)
            .inspect_err(|err| warn!(path = %self.plan.game.display(), "Cut editor could not read the vanilla file: {}", err))
            .ok()
            .and_then(|bytes| Imgcut::parse(&bytes).ok());

        let Some(doc) = parsed else {
            return false;
        };

        self.doc = doc;

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Cut editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Cut editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        match preview::save(&path, &self.doc.write(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Cut editor could not write the file: {}", err);
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

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(usize, usize, String),
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
            Message::Typed(at, cell, typed) => {
                let fits = cell == CUT_NAME_FIELD || numeric(&typed);
                let changed = fits && self.draft.as_mut().is_some_and(|draft| draft.write(at, cell, &typed));

                self.typed = Some((at, cell, typed));

                self.settle(changed, vfs)
            }
            Message::Add => {
                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| {
                    draft.doc.add_cut();

                    true
                });

                self.settle(changed, vfs)
            }
            Message::Remove(at) => {
                if !self.confirm.take(&Intent::Remove(at)) {
                    return self.confirm.set(Intent::Remove(at), Message::ConfirmExpired);
                }

                self.typed = None;

                let changed = self.draft.as_mut().is_some_and(|draft| draft.doc.remove_cut(at).is_some());

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

    fn shown(&self, at: usize, cell: usize, current: String) -> String {
        match &self.typed {
            Some((held_at, held_cell, typed)) if *held_at == at && *held_cell == cell => typed.clone(),
            _ => current,
        }
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut head = row![text(INDEX_LABEL).size(LABEL_SIZE).width(Length::Fixed(INDEX_WIDTH))].spacing(GAP).align_y(Vertical::Center);

        for (cell, label) in CUT_FIELDS.iter().enumerate() {
            let width = if cell == CUT_NAME_FIELD { Length::Fill } else { Length::Fixed(NUMBER_WIDTH) };

            head = head.push(text(*label).size(LABEL_SIZE).width(width));
        }

        head = head.push(iced::widget::Space::new().width(Length::Fixed(REMOVE_WIDTH)));

        let count = draft.doc.count();
        let mut rows = Column::with_capacity(count + 2).spacing(ROW_GAP).width(Length::Fill).push(head);

        for at in 0..count {
            let mut line = row![text(at.to_string()).size(LABEL_SIZE).width(Length::Fixed(INDEX_WIDTH))].spacing(GAP).align_y(Vertical::Center);

            for cell in 0..CUT_FIELDS.len() {
                let named = cell == CUT_NAME_FIELD;
                let current = match named {
                    true => draft.doc.name(at).unwrap_or_default().to_owned(),
                    false => draft.doc.field(at, cell).map_or_else(String::new, |value| value.to_string()),
                };

                line = line.push(
                    text_input("", &self.shown(at, cell, current))
                        .on_input(move |typed| Message::Typed(at, cell, typed))
                        .padding(INPUT_PADDING)
                        .size(INPUT_SIZE)
                        .width(if named { Length::Fill } else { Length::Fixed(NUMBER_WIDTH) })
                        .style(theme::rounded_input),
                );
            }

            let armed = self.confirm.armed_for(&Intent::Remove(at));
            let remove = button(theme::centered_text(if armed { CONFIRM_SHORT_LABEL } else { REMOVE_LABEL }).size(LABEL_SIZE))
                .width(Length::Fixed(REMOVE_WIDTH))
                .padding(SMALL_PADDING)
                .style(theme::danger_button)
                .on_press_maybe((count > 1).then_some(Message::Remove(at)));

            rows = rows.push(line.push(remove));
        }

        let add = button(theme::centered_text(ADD_LABEL).size(LABEL_SIZE)).padding(SMALL_PADDING).style(theme::primary_button).on_press(Message::Add);
        let listing = rows.push(add).push(text(NOTE).size(NOTE_SIZE).style(text::secondary));

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
    use super::numeric;

    #[test]
    fn a_coordinate_takes_whole_numbers_only() {
        assert!(numeric("12") && numeric("-3"));
        assert!(!numeric("") && !numeric("-") && !numeric("1.5") && !numeric("a"));
    }
}
