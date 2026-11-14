use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "What a Cat Base recipe needs before it unlocks\nAny requirement set to -1 is skipped";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 7] =
    ["Unlock ID", "Recipe", "Development Level", "Cleared Stage", "User Rank", "Cannon Attack Level", "Cannon Charge Level"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(-1),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
