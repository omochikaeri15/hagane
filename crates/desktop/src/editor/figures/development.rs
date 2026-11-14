use iced::Element;

pub(crate) use foundry::systems::editor::figures::development::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn cost_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(COST_NOTICE), NOTICE_SIZE, COST_LABELS.len())
}

pub(super) fn tier_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TIER_NOTICE), NOTICE_SIZE, TIER_LABELS.len())
}

pub(super) fn mix_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MIX_NOTICE), NOTICE_SIZE, MIX_LABELS.len())
}

pub(super) fn engineer_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ENGINEER_NOTICE), NOTICE_SIZE, ENGINEER_LABELS.len())
}
