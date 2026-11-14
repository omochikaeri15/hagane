use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const ALTAR_NOTICE: &str = "The enemy castle whose altar this lottery feeds (the same ID the altar rewards use)";
const DRAW_NOTICE: &str = "One step of the Aku Realm lottery, used while the altar is at its unlock level\nEach interval that passes gives the step's draws\nA draw hits at the slot chance and picks one of the stages between the range's bounds";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const ALTAR_LABELS: [&str; 1] = ["Altar Enemy ID"];
pub(super) const CASTLE_LABELS: [&str; 2] = ["Demon Castle Enemy ID", "Sealed Castle Enemy ID"];

const CASTLE_NOTICE: &str = "The enemy a demon castle turns into once its altar has taken its power, found by the castle's own Enemy ID";
pub(super) const DRAW_LABELS: [&str; 6] = ["Unlock Level", "Interval (Minutes)", "Draws", "Slot Chance %", "Range Low", "Range High"];

const CHANCE: usize = 3;
const PERCENT: i32 = 100;

pub(super) fn altar_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn castle_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub(super) fn castle_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(CASTLE_NOTICE), NOTICE_SIZE, CASTLE_LABELS.len())
}

pub(super) fn draw_rule(index: usize) -> Rule {
    match index {
        CHANCE => Rule::Range(0, PERCENT),
        _ => Rule::Floor(0),
    }
}

pub(super) fn altar_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ALTAR_NOTICE), NOTICE_SIZE, ALTAR_LABELS.len())
}

pub(super) fn draw_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(DRAW_NOTICE), NOTICE_SIZE, DRAW_LABELS.len())
}
