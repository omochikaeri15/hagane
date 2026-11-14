use iced::Element;

pub(crate) use foundry::systems::editor::figures::enigma::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn lottery_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LOTTERY_NOTICE), NOTICE_SIZE, LOTTERY_LABELS.len())
}

pub(super) fn rate_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(RATE_NOTICE), NOTICE_SIZE, RATE_LABELS.len())
}

pub(super) fn drop_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(DROP_NOTICE), NOTICE_SIZE, DROP_LABELS.len())
}

pub(super) fn group_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(GROUP_NOTICE), NOTICE_SIZE, GROUP_LABELS.len())
}

pub(super) fn medal_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MEDAL_NOTICE), NOTICE_SIZE, MEDAL_LABELS.len())
}

pub(super) fn time_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TIME_NOTICE), NOTICE_SIZE, TIME_LABELS.len())
}

pub(super) fn rarity_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(RARITY_NOTICE), NOTICE_SIZE, RARITY_LABELS.len())
}
