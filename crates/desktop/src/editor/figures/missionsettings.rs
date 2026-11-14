use iced::Element;

pub(crate) use foundry::systems::editor::figures::missionsettings::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn shortcut_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SHORTCUT_NOTICE), NOTICE_SIZE, SHORTCUT_LABELS.len())
}

pub(super) fn designation_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(DESIGNATION_NOTICE), NOTICE_SIZE, DESIGNATION_LABELS.len())
}

pub(super) fn monthly_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MONTHLY_NOTICE), NOTICE_SIZE, MONTHLY_LABELS.len())
}
