use iced::Element;

pub(crate) use foundry::systems::editor::figures::prizes::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn pattern_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(PATTERN_NOTICE), NOTICE_SIZE, PATTERN_LABELS.len())
}

pub(super) fn reel_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(REEL_NOTICE), NOTICE_SIZE, REEL_LABELS.len())
}
