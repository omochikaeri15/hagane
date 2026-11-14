use iced::Element;

pub(crate) use foundry::systems::editor::figures::gamatoto::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn limit_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LIMIT_NOTICE), NOTICE_SIZE, LIMIT_LABELS.len())
}

pub(super) fn unlock_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(UNLOCK_NOTICE), NOTICE_SIZE, UNLOCK_LABELS.len())
}

pub(super) fn drop_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(DROP_NOTICE), NOTICE_SIZE, DROP_LABELS.len())
}

pub(super) fn area_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(AREA_NOTICE), NOTICE_SIZE, AREA_LABELS.len())
}

pub(super) fn level_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LEVEL_NOTICE), NOTICE_SIZE, LEVEL_LABELS.len())
}

pub(super) fn zone_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ZONE_NOTICE), NOTICE_SIZE, ZONE_LABELS.len())
}

pub(super) fn zone_step_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ZONE_STEP_NOTICE), NOTICE_SIZE, ZONE_STEP_LABELS.len())
}
