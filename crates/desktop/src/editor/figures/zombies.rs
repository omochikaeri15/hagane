use iced::Element;

pub(crate) use foundry::systems::editor::figures::zombies::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn header_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(HEADER_NOTICE), NOTICE_SIZE, HEADER_LABELS.len())
}

pub(super) fn tier_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TIER_NOTICE), NOTICE_SIZE, TIER_LABELS.len())
}
