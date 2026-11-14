use super::resolved::{Choice, Rule};

pub const COST_NOTICE: &str = "What Ototo charges to develop this part by one level, starting from the level this row is numbered after\nMaterials are the item table's 16 material slots in order\nA Cannon file can leave the Z materials off";

pub const TIER_NOTICE: &str = "One stretch of a part's development\nWhile the part is below the level cap, the first unlock decides if it is listed and the second decides if it can be developed\nBoth are a Recipe Unlock";

pub const MIX_NOTICE: &str = "One material Ototo can mix\nSets the item it makes, how long it takes, and what it costs";

pub const COST_LABELS: [&str; 18] = [
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

pub const TIER_LABELS: [&str; 5] = ["Cannon ID", "Part", "Listed By Unlock", "Developed By Unlock", "Up To Level"];

pub const MIX_LABELS: [&str; 5] = ["Made Item ID", "Hours", "Engineers", "Spent Item ID", "Spent Amount"];

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

pub const ENGINEER_NOTICE: &str = "The most Engineers Ototo can hold\nRecruiting stops and the item count is capped at this number";

pub const ENGINEER_LABELS: [&str; 1] = ["Engineer Cap"];
