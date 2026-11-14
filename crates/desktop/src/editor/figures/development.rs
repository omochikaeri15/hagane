use iced::Element;

use super::resolved::{Choice, Rule};
use super::{Draft, Message};

const NOTICE_SIZE: f32 = 11.0;

const COST_NOTICE: &str = "What Ototo charges to develop this part by one level, starting from the level this row is numbered after\nMaterials are the item table's 16 material slots in order\nA Cannon file can leave the Z materials off";

const TIER_NOTICE: &str = "One stretch of a part's development\nWhile the part is below the level cap, the first unlock decides if it is listed and the second decides if it can be developed\nBoth are a Recipe Unlock";

const MIX_NOTICE: &str = "One material Ototo can mix\nSets the item it makes, how long it takes, and what it costs";

pub(super) const COST_LABELS: [&str; 18] = [
    "Hours",
    "Engineers",
    "Bricks",
    "Feathers",
    "Coal",
    "Sprockets",
    "Gold",
    "Meteorite",
    "Beast Bones",
    "Ammonite",
    "Brick Z",
    "Feathers Z",
    "Coal Z",
    "Sprockets Z",
    "Gold Z",
    "Meteorite Z",
    "Beast Bones Z",
    "Ammonite Z",
];

pub(super) const TIER_LABELS: [&str; 5] = ["Cannon ID", "Part", "Listed By Unlock", "Developed By Unlock", "Up To Level"];

pub(super) const MIX_LABELS: [&str; 5] = ["Made Item ID", "Hours", "Engineers", "Spent Item ID", "Spent Amount"];

const PART: usize = 1;

const PARTS: [Choice; 3] = [Choice::new(0, "Cannon"), Choice::new(1, "Foundation"), Choice::new(2, "Style")];

pub(super) fn cost_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn tier_rule(index: usize) -> Rule {
    match index {
        PART => Rule::Choice(&PARTS),
        _ => Rule::Floor(0),
    }
}

pub(super) fn mix_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub(super) fn cost_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(COST_NOTICE), NOTICE_SIZE, COST_LABELS.len())
}

pub(super) fn tier_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(TIER_NOTICE), NOTICE_SIZE, TIER_LABELS.len())
}

pub(super) fn mix_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MIX_NOTICE), NOTICE_SIZE, MIX_LABELS.len())
}

const ENGINEER_NOTICE: &str = "The most Engineers Ototo can hold\nRecruiting stops and the item count is capped at this number";

pub(super) const ENGINEER_LABELS: [&str; 1] = ["Engineer Cap"];

pub(super) fn engineer_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(ENGINEER_NOTICE), NOTICE_SIZE, ENGINEER_LABELS.len())
}
