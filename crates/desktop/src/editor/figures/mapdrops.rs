
use iced::Element;

pub(crate) use foundry::systems::editor::figures::mapdrops::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::searched(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE)
}

pub(super) fn chara_view<'a>(draft: &'a Draft, width: f32, armed: bool) -> Element<'a, Message> {
    super::unitbuy::searched(draft, width, "", armed, Some(CHARA_NOTICE), NOTICE_SIZE)
}
