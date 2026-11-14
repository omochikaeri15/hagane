use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const HEADER_NOTICE: &str = "The lottery's first line\nEvery interval, the game rolls each tier you have reached";
const TIER_NOTICE: &str = "A tier applies once the map has this many cleared zombie stages\nEach interval it rolls this many draws at this chance, and always gives at least the minimum";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const HEADER_LABELS: [&str; 2] = ["Lottery ID", "Interval (Minutes)"];
pub(super) const TIER_LABELS: [&str; 4] = ["Cleared Zombie Stages", "Draws", "Chance %", "Minimum Granted"];

const CHANCE: usize = 2;
const PERCENT: i32 = 100;

pub(super) fn header_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(1),
    }
}

pub(super) fn tier_rule(index: usize) -> Rule {
    match index {
        CHANCE => Rule::Range(0, PERCENT),
        _ => Rule::Floor(0),
    }
}

pub(super) fn header_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(HEADER_NOTICE), NOTICE_SIZE, HEADER_LABELS.len())
}

pub(super) fn tier_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TIER_NOTICE), NOTICE_SIZE, TIER_LABELS.len())
}
