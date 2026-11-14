use super::resolved::Rule;

pub const ALTAR_NOTICE: &str = "The enemy castle whose altar this lottery feeds (the same ID the altar rewards use)";

pub const DRAW_NOTICE: &str = "One step of the Aku Realm lottery, used while the altar is at its unlock level\nEach interval that passes gives the step's draws\nA draw hits at the slot chance and picks one of the stages between the range's bounds";

pub const ALTAR_LABELS: [&str; 1] = ["Altar Enemy ID"];

pub const CASTLE_LABELS: [&str; 2] = ["Demon Castle Enemy ID", "Sealed Castle Enemy ID"];

pub const CASTLE_NOTICE: &str = "The enemy a demon castle turns into once its altar has taken its power, found by the castle's own Enemy ID";

pub const DRAW_LABELS: [&str; 6] = ["Unlock Level", "Interval (Minutes)", "Draws", "Slot Chance %", "Range Low", "Range High"];

pub(super) const CHANCE: usize = 3;

pub(super) const PERCENT: i32 = 100;

pub(super) fn altar_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn castle_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub(super) fn draw_rule(index: usize) -> Rule {
    match index {
        CHANCE => Rule::Range(0, PERCENT),
        _ => Rule::Floor(0),
    }
}
