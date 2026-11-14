use super::resolved::Rule;

pub const LOTTERY_NOTICE: &str = "One enigma lottery\nOnly runs while its event is scheduled and its stage (if it has one) is cleared\nA discovery picks one of its maps by weight, and the map takes the appraisal time to open";

pub const RATE_NOTICE: &str = "Discovery chances at one Discovery level\nEach step is rolled once, when the energy spent since the last discovery reaches it";

pub const DROP_NOTICE: &str = "Which lotteries a discovery picks from at one Discovery level, by weight\nA lottery is the row number of the lottery table, counted from 0";

pub const GROUP_NOTICE: &str = "Lotteries tried in order when this stage is cleared, each at its own chance out of 100\nThis happens before the energy steps are checked";

pub const MEDAL_NOTICE: &str = "A medal that raises the Discovery level by 1 once you earn it";

pub const LOTTERY_LABELS: [&str; 23] = [
    "Rarity",
    "Appraisal Minutes",
    "Needs Stage Cleared",
    "Map 1 ID",
    "Map 1 Weight",
    "Map 2 ID",
    "Map 2 Weight",
    "Map 3 ID",
    "Map 3 Weight",
    "Map 4 ID",
    "Map 4 Weight",
    "Map 5 ID",
    "Map 5 Weight",
    "Map 6 ID",
    "Map 6 Weight",
    "Map 7 ID",
    "Map 7 Weight",
    "Map 8 ID",
    "Map 8 Weight",
    "Map 9 ID",
    "Map 9 Weight",
    "Map 10 ID",
    "Map 10 Weight",
];

pub const RATE_LABELS: [&str; 32] = [
    "Step 1 Energy Spent",
    "Step 1 Chance (of 1000)",
    "Step 2 Energy Spent",
    "Step 2 Chance (of 1000)",
    "Step 3 Energy Spent",
    "Step 3 Chance (of 1000)",
    "Step 4 Energy Spent",
    "Step 4 Chance (of 1000)",
    "Step 5 Energy Spent",
    "Step 5 Chance (of 1000)",
    "Step 6 Energy Spent",
    "Step 6 Chance (of 1000)",
    "Step 7 Energy Spent",
    "Step 7 Chance (of 1000)",
    "Step 8 Energy Spent",
    "Step 8 Chance (of 1000)",
    "Step 9 Energy Spent",
    "Step 9 Chance (of 1000)",
    "Step 10 Energy Spent",
    "Step 10 Chance (of 1000)",
    "Step 11 Energy Spent",
    "Step 11 Chance (of 1000)",
    "Step 12 Energy Spent",
    "Step 12 Chance (of 1000)",
    "Step 13 Energy Spent",
    "Step 13 Chance (of 1000)",
    "Step 14 Energy Spent",
    "Step 14 Chance (of 1000)",
    "Step 15 Energy Spent",
    "Step 15 Chance (of 1000)",
    "Step 16 Energy Spent",
    "Step 16 Chance (of 1000)",
];

pub const DROP_LABELS: [&str; 24] = [
    "Lottery 1",
    "Lottery 1 Weight",
    "Lottery 2",
    "Lottery 2 Weight",
    "Lottery 3",
    "Lottery 3 Weight",
    "Lottery 4",
    "Lottery 4 Weight",
    "Lottery 5",
    "Lottery 5 Weight",
    "Lottery 6",
    "Lottery 6 Weight",
    "Lottery 7",
    "Lottery 7 Weight",
    "Lottery 8",
    "Lottery 8 Weight",
    "Lottery 9",
    "Lottery 9 Weight",
    "Lottery 10",
    "Lottery 10 Weight",
    "Lottery 11",
    "Lottery 11 Weight",
    "Lottery 12",
    "Lottery 12 Weight",
];

pub const GROUP_LABELS: [&str; 14] = [
    "Map ID",
    "Stage",
    "Lottery 1",
    "Lottery 1 Chance (%)",
    "Lottery 2",
    "Lottery 2 Chance (%)",
    "Lottery 3",
    "Lottery 3 Chance (%)",
    "Lottery 4",
    "Lottery 4 Chance (%)",
    "Lottery 5",
    "Lottery 5 Chance (%)",
    "Lottery 6",
    "Lottery 6 Chance (%)",
];

pub const MEDAL_LABELS: [&str; 1] = ["Medal ID"];

pub(super) const THOUSAND: i32 = 1000;

pub(super) const PERCENT: i32 = 100;

const GROUP_PAIRS: usize = 2;

pub(super) fn plain_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn rate_rule(index: usize) -> Rule {
    match index % 2 {
        1 => Rule::Range(0, THOUSAND),
        _ => Rule::Floor(0),
    }
}

pub(super) fn group_rule(index: usize) -> Rule {
    match index >= GROUP_PAIRS && (index - GROUP_PAIRS) % 2 == 1 {
        true => Rule::Range(0, PERCENT),
        false => Rule::Floor(0),
    }
}

pub const TIME_NOTICE: &str = "How many minutes this enigma map stays on the list after it opens\nRows follow the enigma maps in ID order, starting from the first";

pub const TIME_LABELS: [&str; 1] = ["Playable Minutes"];

pub const RARITY_NOTICE: &str = "One enigma rarity, as a lottery names it\nThe text label is the key the discovery message uses to find its name\nThe cut is the badge drawn on the enigma list";

pub const RARITY_LABELS: [&str; 3] = ["Rarity ID", "Text Label", "Cut"];

const RARITY_CUT: usize = 2;

pub(super) fn rarity_rule(index: usize) -> Rule {
    match index {
        RARITY_CUT => Rule::Floor(0),
        _ => Rule::Opaque,
    }
}
