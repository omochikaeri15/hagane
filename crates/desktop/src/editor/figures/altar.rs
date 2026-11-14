use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "Clearing this stage raises the altar enemy's level cap by this amount, and can unseal its altar\nThe row is found by (Map ID * 100) + Stage";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 4] = ["Stage Key", "Level Cap Raise", "Unseals Altar", "Altar Enemy ID"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        2 => Rule::Flag,
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
