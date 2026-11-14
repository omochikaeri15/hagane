use iced::Element;

pub(crate) use foundry::systems::editor::figures::shrine::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn offering_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(OFFERING_NOTICE), NOTICE_SIZE, OFFERING_LABELS.len())
}

pub(super) fn extra_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(EXTRA_NOTICE), NOTICE_SIZE, EXTRA_LABELS.len())
}

pub(super) fn fortune_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(FORTUNE_NOTICE), NOTICE_SIZE, FORTUNE_LABELS.len())
}
