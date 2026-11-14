use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const STAGE_NOTICE: &str = "A Legend Quest stage\nBoth unlock conditions must be met to enter it\nNo Continues takes the continue away";
const LOTTERY_NOTICE: &str = "How many lineup slots a Legend Quest stage hands out, picked by weight\nEach row is a clear count, and the last row covers every clear past it";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const STAGE_LABELS: [&str; 9] =
    ["Unlock Condition 1", "Unlock Condition 2", "Column 3", "Column 4", "Column 5", "Column 6", "Column 7", "Column 8", "No Continues"];

pub(super) const LOTTERY_LABELS: [&str; 11] =
    ["10 Slots", "9 Slots", "8 Slots", "7 Slots", "6 Slots", "5 Slots", "4 Slots", "3 Slots", "2 Slots", "1 Slot", "0 Slots"];

const NO_CONTINUES: usize = 8;

pub(super) fn stage_rule(index: usize) -> Rule {
    match index {
        NO_CONTINUES => Rule::Flag,
        _ => Rule::Plain,
    }
}

pub(super) fn lottery_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn stage_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(STAGE_NOTICE), NOTICE_SIZE, STAGE_LABELS.len())
}

pub(super) fn lottery_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(LOTTERY_NOTICE), NOTICE_SIZE, LOTTERY_LABELS.len())
}
