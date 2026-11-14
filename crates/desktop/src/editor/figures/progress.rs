use iced::Element;

pub(crate) use foundry::systems::editor::figures::progress::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn stamp_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(STAMP_NOTICE), NOTICE_SIZE, STAMP_LABELS.len())
}

pub(super) fn slot_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SLOT_NOTICE), NOTICE_SIZE, SLOT_LABELS.len())
}
