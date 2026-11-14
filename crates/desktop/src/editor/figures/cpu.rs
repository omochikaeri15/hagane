use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One price bracket of the Super CPU\nUses count from 1, and the bracket holding the next use sets how many Cat CPU items it costs\nIt uses the plain cost, or the pass cost while an Officer's Club pass is active\nThe highest last use across all brackets is the limit\nA use that no bracket holds costs 9999";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 4] = ["First Use", "Last Use", "Cost", "Cost With Pass"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Floor(1),
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
