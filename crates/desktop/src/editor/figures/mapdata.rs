
use iced::alignment::Horizontal;
use iced::widget::{column, container, text};
use iced::{Element, Length};

pub(crate) use foundry::systems::editor::figures::mapdata::*;

use super::{cards, Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

const CHROME_GAP: f32 = 8.0;

// The map-wide cells sit above the scroll because they belong to the file rather than to
// the stage, and the stage's own row scrolls beneath them.
pub(super) fn view<'a>(draft: &'a Draft, width: f32, armed: bool) -> Element<'a, Message> {
    let lead = draft.lead();
    let chrome: Vec<usize> = (0..lead).collect();
    let shown: Vec<usize> = (lead..draft.len()).collect();

    let notice = container(text(NOTICE).size(NOTICE_SIZE).align_x(Horizontal::Center).style(text::secondary))
        .width(Length::Fill)
        .center_x(Length::Fill);

    let top = column![cards::band(draft, width, &chrome), notice].spacing(CHROME_GAP);

    cards::shell(
        Some(cards::header(top)),
        cards::grid(draft, width, &shown, None),
        cards::footer(vec![cards::sync(armed)]),
    )
}
