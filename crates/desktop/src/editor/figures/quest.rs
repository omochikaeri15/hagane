use iced::Element;

pub(crate) use foundry::systems::editor::figures::quest::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn stage_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(STAGE_NOTICE), NOTICE_SIZE, STAGE_LABELS.len())
}

pub(super) fn lottery_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LOTTERY_NOTICE), NOTICE_SIZE, LOTTERY_LABELS.len())
}
