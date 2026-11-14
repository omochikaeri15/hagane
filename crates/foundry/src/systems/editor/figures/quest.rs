use super::resolved::Rule;

pub const STAGE_NOTICE: &str = "A Legend Quest stage\nBoth unlock conditions must be met to enter it\nNo Continues takes the continue away";

pub const LOTTERY_NOTICE: &str = "How many lineup slots a Legend Quest stage hands out, picked by weight\nEach row is a clear count, and the last row covers every clear past it";

pub const STAGE_LABELS: [&str; 9] =
    ["Unlock Condition 1", "Unlock Condition 2", "Column 3", "Column 4", "Column 5", "Column 6", "Column 7", "Column 8", "No Continues"];

pub const LOTTERY_LABELS: [&str; 11] =
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
