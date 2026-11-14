use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per power up, in the game's own order\nMax Level and Max Plus Level are also the caps a fixed lineup is checked against, so lowering them can stop a preset from loading";

const NOTICE_SIZE: f32 = 11.0;

const TIER_FLOOR: i32 = 0;
const TIER_CEILING: i32 = 3;

pub(super) const NAMES: &[(&str, &str)] = &[
    ("xp_price", "Base XP Cost"),
    ("rarity", "Cost Tier (N, R, SR, ER)"),
    ("level_cap", "Max Level"),
    ("plus_cap", "Max Plus Level"),
    ("story_level_cap", "Max Level in Chapters 1-2"),
];

pub(super) fn rule(field: &str) -> Option<Rule> {
    match field {
        "xp_price" | "plus_cap" | "story_level_cap" => Some(Rule::Floor(0)),
        "level_cap" => Some(Rule::Floor(1)),
        "rarity" => Some(Rule::Range(TIER_FLOOR, TIER_CEILING)),
        _ => None,
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, draft.schema().known())
}
