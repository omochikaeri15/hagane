use iced::widget::container;
use iced::{Element, Length, Size, Task};

use emulator::runtime::VERSION;

use crate::app::theme;
use crate::widget::popup;

use super::Mark;

const SIZE: Size = Size::new(420.0, 180.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Unused, SIZE);

pub(super) const TITLE: &str = "Unused File";
pub(super) const FOLDER: &str = "Unused";
pub(super) const FILES: [(&str, Fate); 4] = [
    ("AbilityLimit.csv", Fate::Unread),
    ("point_condition.csv", Fate::Unread),
    ("rewardStage.csv", Fate::Unheeded),
    ("equipment_filter.tsv", Fate::Unheeded),
];
pub(super) const FAMILIES: [(&str, &str, &str); 1] = [("gatyachara_", "_z.png", "Black Gacha Images")];

const NOTICE_SIZE: f32 = 14.0;
const BODY_PADDING: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fate {
    Unread,
    Unheeded,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    fate: Fate,
}

pub(super) fn plan(label: String, file: &str) -> Plan {
    let fate = FILES.into_iter().find(|(name, _)| *name == file).map_or(Fate::Unread, |(_, fate)| fate);

    Plan { label, fate }
}

pub(super) fn notice(fate: Fate) -> String {
    match fate {
        Fate::Unread => format!("Cat game version {VERSION} never reads this file."),
        Fate::Unheeded => format!("Cat game version {VERSION} loads this file but never uses what it holds."),
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
}

#[derive(Default)]
pub(super) struct State {
    plan: Option<Plan>,
    frame: popup::State,
    notice: String,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize) {
        self.frame = popup::cascaded(nudge);
        self.notice = notice(plan.fate);
        self.plan = Some(plan);
    }

    pub(super) fn drafting(&self) -> bool {
        self.plan.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>) {
        if self.plan.is_some() {
            self.plan = plan;
        }
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        let Message::Popup(msg) = message;

        if self.frame.update(msg, POPUP) {
            self.plan = None;
        }

        Task::none()
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let plan = self.plan.as_ref()?;

        Some(self.frame.view(&plan.label, POPUP, window, Message::Popup, move || self.body(), None))
    }

    fn body(&self) -> Element<'_, Message> {
        container(theme::centered_text(self.notice.as_str()).size(NOTICE_SIZE))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(BODY_PADDING)
            .into()
    }
}

pub(super) fn mark() -> Mark {
    Mark::Same
}

#[cfg(test)]
mod tests {
    use super::{notice, plan, Fate, State};

    #[test]
    fn the_notice_names_a_game_version() {
        let said = notice(Fate::Unread);
        let version = said.trim_start_matches("Cat game version ").trim_end_matches(" never reads this file.");

        assert!(said.starts_with("Cat game version ") && said.ends_with(" never reads this file."));
        assert!(version.split('.').count() >= 2 && version.split('.').all(|part| part.parse::<u32>().is_ok()));
    }

    #[test]
    fn a_file_the_game_loads_and_ignores_says_so() {
        let loaded = notice(plan("a".to_owned(), "rewardStage.csv").fate);

        assert!(loaded.contains("loads this file") && !loaded.contains("never reads"));
    }

    #[test]
    fn an_opened_file_stays_open_until_its_pin_goes() {
        let mut state = State::default();

        state.sync(Some(plan("a".to_owned(), "a")));
        assert!(!state.drafting());

        state.begin(plan("a".to_owned(), "a"), 0);
        assert!(state.drafting());

        state.sync(None);
        assert!(!state.drafting());
    }
}
