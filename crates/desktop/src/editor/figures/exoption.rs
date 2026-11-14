use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "Clearing this map opens the special EX map named here\nThe game stops reading this file at its first comment line, so only rows above the comments can be changed";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 2] = ["Map ID", "EX Map ID"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
