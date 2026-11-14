use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const STAMP_NOTICE: &str = "One row per day of the login stamp card, in day order\nSets what kind of reward that day gives and how much";
const SLOT_NOTICE: &str = "One row per lineup slot, in slot order\n-1 leaves the slot without a price";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const STAMP_LABELS: [&str; 2] = ["Reward Type", "Amount"];
pub(super) const SLOT_LABELS: [&str; 2] = ["Unlock Option", "Quantity"];

pub(super) fn stamp_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn slot_rule() -> Rule {
    Rule::Floor(-1)
}

pub(super) fn stamp_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(STAMP_NOTICE), NOTICE_SIZE, STAMP_LABELS.len())
}

pub(super) fn slot_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SLOT_NOTICE), NOTICE_SIZE, SLOT_LABELS.len())
}
