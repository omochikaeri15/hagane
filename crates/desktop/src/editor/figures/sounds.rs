use iced::Element;

pub(crate) use foundry::systems::editor::figures::sounds::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn sound_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SOUND_NOTICE), NOTICE_SIZE, SOUND_LABELS.len())
}

pub(super) fn chest_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(CHEST_NOTICE), NOTICE_SIZE, CHEST_LABELS.len())
}
