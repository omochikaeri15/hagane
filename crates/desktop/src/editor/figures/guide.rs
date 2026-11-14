use iced::Element;

pub(crate) use foundry::systems::editor::figures::guide::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn guide_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(GUIDE_NOTICE), NOTICE_SIZE, GUIDE_LABELS.len())
}

pub(super) fn limit_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LIMIT_NOTICE), NOTICE_SIZE, LIMIT_LABELS.len())
}

pub(super) fn enemy_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ENEMY_NOTICE), NOTICE_SIZE, ENEMY_LABELS.len())
}

pub(super) fn filter_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(FILTER_NOTICE), NOTICE_SIZE, FILTER_LABELS.len())
}

pub(super) fn exclusion_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(EXCLUSION_NOTICE), NOTICE_SIZE, EXCLUSION_LABELS.len())
}

pub(super) fn marker_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MARKER_NOTICE), NOTICE_SIZE, MARKER_LABELS.len())
}

pub(super) fn orb_choice_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ORB_CHOICE_NOTICE), NOTICE_SIZE, ORB_CHOICE_LABELS.len())
}

pub(super) fn level_up_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LEVEL_UP_NOTICE), NOTICE_SIZE, LEVEL_UP_LABELS.len())
}

pub(super) fn recommend_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(RECOMMEND_NOTICE), NOTICE_SIZE, RECOMMEND_LABELS.len())
}

pub(super) fn counter_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(COUNTER_NOTICE), NOTICE_SIZE, COUNTER_LABELS.len())
}

pub(super) fn orb_option_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ORB_OPTION_NOTICE), NOTICE_SIZE, ORB_OPTION_LABELS.len())
}

pub(super) fn refund_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(REFUND_NOTICE), NOTICE_SIZE, REFUND_LABELS.len())
}

pub(super) fn shortcut_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SHORTCUT_NOTICE), NOTICE_SIZE, SHORTCUT_LABELS.len())
}

pub(super) fn fanfare_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(FANFARE_NOTICE), NOTICE_SIZE, FANFARE_LABELS.len())
}

pub(super) fn spotlight_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SPOTLIGHT_NOTICE), NOTICE_SIZE, SPOTLIGHT_LABELS.len())
}

pub(super) fn tip_setting_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TIP_SETTING_NOTICE), NOTICE_SIZE, TIP_SETTING_LABELS.len())
}

pub(super) fn rating_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(RATING_NOTICE), NOTICE_SIZE, RATING_LABELS.len())
}

pub(super) fn band_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(BAND_NOTICE), NOTICE_SIZE, BAND_LABELS.len())
}

pub(super) fn rank_group_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(RANK_GROUP_NOTICE), NOTICE_SIZE, RANK_GROUP_LABELS.len())
}

pub(super) fn ability_icon_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ABILITY_ICON_NOTICE), NOTICE_SIZE, ABILITY_ICON_LABELS.len())
}

pub(super) fn trait_icon_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TRAIT_ICON_NOTICE), NOTICE_SIZE, TRAIT_ICON_LABELS.len())
}

pub(super) fn menu_text_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MENU_TEXT_NOTICE), NOTICE_SIZE, MENU_TEXT_LABELS.len())
}
