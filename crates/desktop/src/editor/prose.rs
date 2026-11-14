use std::path::PathBuf;

use iced::alignment::Horizontal;
use iced::widget::{button, column, container, pick_list, scrollable, text, text_input};
use iced::{Element, Length, Padding, Size, Task};

use foundry::common::preview::{self, Stamp};
use foundry::Vfs;

pub(crate) use foundry::systems::editor::prose::*;

use crate::app::{theme, Page};
use crate::common::feedback::{Slot, CONFIRM_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::Paged;

const FIELD_HEIGHT: f32 = INPUT_SIZE * 1.3 + PADDING * 2.0;

const SYNC_HEIGHT: f32 = INPUT_SIZE * 1.3 + (PADDING + 1.0) * 2.0;

const POPUP_WIDTH: f32 = 420.0;

const NARROW_WIDTH: f32 = POPUP_WIDTH * 0.6;

const NOTICE_SIZE: f32 = 11.0;

const NOTICE_LINES: f32 = 2.0;

const NOTICE_HEIGHT: f32 = NOTICE_SIZE * 1.3 * NOTICE_LINES;

const INPUT_SIZE: f32 = 13.0;

const PADDING: f32 = 4.0;

const GAP: f32 = 8.0;

const BODY_PADDING: f32 = 12.0;

const SYNC_WIDTH: f32 = 172.0;

pub(super) trait Framed {
    fn kind(self) -> popup::Kind;
    fn chooses(self) -> f32;
    fn width(self) -> f32;
}

impl Framed for Subject {
    fn kind(self) -> popup::Kind {
        match self {
            Self::Explanation => popup::Kind::Explanation,
            Self::EnemyName => popup::Kind::EnemyName,
            Self::EnemyDescription => popup::Kind::EnemyDescription,
            Self::ComboName => popup::Kind::ComboName,
            Self::TalentText => popup::Kind::TalentText,
            Self::MapName => popup::Kind::MapName,
            Self::StageName => popup::Kind::StageName,
            Self::ItemName => popup::Kind::ItemName,
            Self::CatGlossary => popup::Kind::CatGlossary,
            Self::EnemyGlossary => popup::Kind::EnemyGlossary,
        }
    }

    fn chooses(self) -> f32 {
        f32::from(u8::from(matches!(self, Self::TalentText)))
    }

    fn width(self) -> f32 {
        match self {
            Self::EnemyName | Self::ComboName | Self::MapName | Self::StageName => NARROW_WIDTH,
            Self::Explanation | Self::EnemyDescription | Self::TalentText | Self::ItemName | Self::CatGlossary | Self::EnemyGlossary => POPUP_WIDTH,
        }
    }
}

impl Paged for Subject {
    fn page(self) -> Page {
        super::paged(self.realm())
    }
}

fn spec(subject: Subject) -> popup::Spec {
    let fields = subject.labels().len() as f32 + subject.chooses();
    let height = FIELD_HEIGHT * fields
        + GAP * (fields - 1.0)
        + SYNC_HEIGHT
        + GAP
        + BODY_PADDING * 3.0
        + subject.notice().map_or(0.0, |_| NOTICE_HEIGHT + GAP)
        + popup::CHROME_HEIGHT;

    popup::Spec::new(subject.kind(), Size::new(subject.width(), height))
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Aimed(usize),
    Changed(usize, String),
    Sync,
    SyncExpired,
    Persisted(u64, PathBuf, Option<Stamp>),
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    confirm: Slot<()>,
    pinned: Option<usize>,
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.confirm.expire();
        self.reload(plan, vfs);
    }

    fn reload(&mut self, plan: Plan, vfs: &Vfs) {
        self.draft = Draft::load(plan, vfs);
        self.pinned = self
            .draft
            .as_ref()
            .filter(|draft| draft.plan.subject.pins())
            .map(|draft| draft.plan.row);
    }

    pub(super) fn flush(&mut self, vfs: &Vfs) -> Task<Message> {
        self.draft.as_mut().map_or(Task::none(), |draft| draft.persist_if_dirty(vfs))
    }

    pub(super) fn flush_now(&mut self, vfs: &Vfs) {
        if let Some(draft) = self.draft.as_mut() {
            draft.persist_now(vfs);
        }
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| preview::stamp(&draft.read_from) != Some(draft.stamp))
    }

    pub(super) fn sync(&mut self, plans: &[Plan], vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        if current.dirty || current.writing {
            return;
        }

        let Some(plan) = plans.iter().find(|plan| plan.file == current.plan.file).or_else(|| plans.first()) else {
            self.draft = None;

            return;
        };

        if current.plan.target_mod.is_none() != plan.target_mod.is_none() {
            self.draft = None;

            return;
        }

        let held = self.pinned.filter(|row| plan.rows.is_empty() || plan.rows.contains(row));

        let plan = match held {
            Some(row) => Plan { row, ..plan.clone() },
            None => plan.clone(),
        };

        if !current.plan.matches(&plan) || preview::stamp(&current.read_from) != Some(current.stamp) {
            self.reload(plan, vfs);
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                let Some(subject) = self.draft.as_ref().map(|draft| draft.plan.subject) else {
                    return Task::none();
                };

                if self.frame.update(msg, spec(subject)) {
                    let task = self.draft.as_mut().map_or(Task::none(), |draft| draft.persist_if_dirty(vfs));

                    self.draft = None;
                    self.confirm.expire();

                    return task;
                }
            }
            Message::Aimed(row) => {
                let Some(draft) = self.draft.as_mut().filter(|draft| draft.plan.row != row) else {
                    return Task::none();
                };

                draft.persist_now(vfs);

                let plan = Plan { row, ..draft.plan.clone() };
                self.reload(plan, vfs);
            }
            Message::Changed(index, value) => {
                if let Some(draft) = self.draft.as_mut() {
                    draft.edit(index, value);
                }
            }
            Message::SyncExpired => self.confirm.expire(),
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::SyncExpired);
                }

                if let Some(draft) = self.draft.as_mut() {
                    draft.restore();
                }
            }
            Message::Persisted(token, path, stamp) => {
                if let Some(draft) = self.draft.as_mut()
                    && draft.token == token
                {
                    draft.writing = false;

                    match stamp {
                        Some(stamp) => {
                            draft.read_from = path;
                            draft.stamp = stamp;
                            draft.failed = false;
                        }
                        None => draft.failed = true,
                    }
                }
            }
        }

        self.draft.as_mut().map_or(Task::none(), |draft| draft.persist_if_dirty(vfs))
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;
        let armed = self.confirm.is_set();

        Some(self.frame.view(
            &draft.plan.label,
            spec(draft.plan.subject),
            window,
            Message::Popup,
            move || draft.body(armed),
            None,
        ))
    }
}

trait Shown {
    fn persist_if_dirty(&mut self, vfs: &Vfs) -> Task<Message>;
    fn persist(&mut self, vfs: &Vfs) -> Task<Message>;
    fn picker(&self) -> Option<Element<'_, Message>>;
    fn body(&self, armed: bool) -> Element<'_, Message>;
}

impl Shown for Draft {
    fn persist_if_dirty(&mut self, vfs: &Vfs) -> Task<Message> {
        if !self.dirty || self.writing {
            return Task::none();
        }

        self.persist(vfs)
    }

    fn persist(&mut self, vfs: &Vfs) -> Task<Message> {
        let Some((path, body, stamp, token)) = self.prepare_write(vfs) else {
            return Task::none();
        };

        let reported = path.clone();

        Task::perform(
            smol::unblock(move || write_now(&path, &body, stamp)),
            move |stamp| Message::Persisted(token, reported.clone(), stamp),
        )
    }

    fn picker(&self) -> Option<Element<'_, Message>> {
        if self.plan.rows.len() < 2 {
            return None;
        }

        let held: Vec<Aim> = self.plan.rows.iter().map(|row| self.aim(*row)).collect();
        let current = held.iter().find(|aim| aim.row == self.row).cloned()?;

        Some(
            pick_list(held, Some(current), |aim: Aim| Message::Aimed(aim.row))
                .width(Length::Fill)
                .padding(PADDING)
                .text_size(INPUT_SIZE)
                .style(theme::combo_box)
                .menu_style(theme::combo_box_menu)
                .into(),
        )
    }

    fn body(&self, armed: bool) -> Element<'_, Message> {
        let mut rows = column![].spacing(GAP);

        if let Some(notice) = self.plan.subject.notice() {
            rows = rows.push(
                text(notice)
                    .size(NOTICE_SIZE)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .style(text::secondary),
            );
        }

        if let Some(picker) = self.picker() {
            rows = rows.push(picker);
        }

        for (index, label) in self.plan.subject.labels().iter().enumerate() {
            let field = text_input(label, self.fields.get(index).map_or("", String::as_str))
                .on_input(move |value| Message::Changed(index, value))
                .size(INPUT_SIZE)
                .padding(PADDING)
                .width(Length::Fill)
                .style(theme::rounded_input);

            rows = rows.push(field);
        }

        let label = if armed { CONFIRM_LABEL } else { "Sync With \"game\"" };

        let sync = button(theme::centered_text(label).size(INPUT_SIZE).wrapping(iced::widget::text::Wrapping::None))
            .width(Length::Fixed(SYNC_WIDTH))
            .padding([PADDING + 1.0, 10.0])
            .style(theme::danger_button)
            .on_press(Message::Sync);

        column![
            smooth_scroll(
                scrollable(container(rows).padding(BODY_PADDING).width(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fill),
            ),
            container(sync)
                .width(Length::Fill)
                .center_x(Length::Fill)
                .padding(Padding::ZERO.top(GAP).left(BODY_PADDING).right(BODY_PADDING).bottom(BODY_PADDING)),
        ]
        .height(Length::Fill)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::{join, row, Subject, TALENT_BREAK};

    const PIPE: Option<char> = Some('|');

    // textID|text, with at most one <br> in the text across every shipped locale.
    // The id is skipped so it cannot be typed over, and the break becomes the second field.
    #[test]
    fn a_description_splits_its_id_off_and_its_break_into_a_second_line() {
        let line = "12|Gain the \"Weaken\" ability.<br>Level up to Weaken enemies for even longer!";
        let (head, fields, tail) = row(line, PIPE, 1, 2, Some(TALENT_BREAK));

        assert_eq!(head, ["12"]);
        assert_eq!(fields[0], "Gain the \"Weaken\" ability.");
        assert_eq!(fields[1], "Level up to Weaken enemies for even longer!");
        assert!(tail.is_empty());

        assert_eq!(join(&head, &fields, &tail, PIPE, Some(TALENT_BREAK)), line);
    }

    #[test]
    fn a_description_with_one_line_does_not_grow_a_break() {
        let line = "4|Gain the \"Only Attacks\" ability.";
        let (head, fields, tail) = row(line, PIPE, 1, 2, Some(TALENT_BREAK));

        assert_eq!(fields[1], "", "the second line is empty, not the whole text again");
        assert_eq!(join(&head, &fields, &tail, PIPE, Some(TALENT_BREAK)), line, "and no <br> is added");
    }

    // The file is roster-wide: one text id is referenced by talents on unrelated units,
    // so the popup has to say so the way the cost curves do.
    #[test]
    fn only_the_roster_wide_subject_carries_a_notice() {
        assert!(Subject::TalentText.notice().is_some());

        for subject in [Subject::Explanation, Subject::EnemyName, Subject::EnemyDescription, Subject::ComboName] {
            assert_eq!(subject.notice(), None, "{subject:?} edits a row that belongs to what it names");
        }
    }

    // A glossary line keeps the name in its corner brackets and the terminator behind
    // the explanation, and an emptied trailing line drops out rather than leaving a gap.
    #[test]
    fn a_glossary_line_round_trips_its_brackets_and_terminator() {
        let line = "【Freeze】|Indicated enemy types will become unable to|move for a set amount of time.|＠";
        let (head, fields, tail) = super::glossary_row(line, '|');

        assert!(head.is_empty());
        assert_eq!(fields, ["Freeze", "Indicated enemy types will become unable to", "move for a set amount of time.", ""]);
        assert_eq!(tail, ["＠"]);
        assert_eq!(super::glossary_join(&fields, &tail, '|'), line);

        let shortened = ["Freeze".to_owned(), "One line.".to_owned(), String::new(), String::new()];
        assert_eq!(super::glossary_join(&shortened, &tail, '|'), "【Freeze】|One line.|＠");
    }

    #[test]
    fn only_the_talent_text_is_broken_on_a_marker() {
        assert_eq!(Subject::TalentText.wrapped(), Some(TALENT_BREAK));

        for subject in [Subject::Explanation, Subject::EnemyName, Subject::EnemyDescription, Subject::ComboName] {
            assert_eq!(subject.wrapped(), None, "{subject:?} has no inner break");
        }
    }
}
