use iced::Element;

pub(crate) use foundry::systems::editor::figures::comboparams::*;

use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    let held = draft.stored.clamp(1, BANDS.len());

    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, held)
}
