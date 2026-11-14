use iced::Element;

pub(crate) use foundry::systems::editor::figures::conditions::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    let reach = (draft.stored + 1).clamp(HEAD.len(), WIDTH);

    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, reach)
}

pub(super) fn group_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    let reach = (draft.stored + 1).clamp(1, GROUP_WIDTH);

    super::unitbuy::bounded(draft, width, query, armed, Some(GROUP_NOTICE), NOTICE_SIZE, reach)
}
