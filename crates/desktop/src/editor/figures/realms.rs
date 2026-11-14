use iced::Element;

pub(crate) use foundry::systems::editor::figures::realms::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn castle_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(CASTLE_NOTICE), NOTICE_SIZE, CASTLE_LABELS.len())
}

pub(super) fn altar_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ALTAR_NOTICE), NOTICE_SIZE, ALTAR_LABELS.len())
}

pub(super) fn draw_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(DRAW_NOTICE), NOTICE_SIZE, DRAW_LABELS.len())
}
