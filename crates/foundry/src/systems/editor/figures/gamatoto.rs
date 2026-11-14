use super::resolved::{Choice, Rule};

pub const LIMIT_NOTICE: &str = "Gamatoto's limits\nSets the highest level he can reach, how many areas the destination list can hold, and how many helpers he can keep";

pub const UNLOCK_NOTICE: &str = "What an expedition area needs to unlock (a User Rank and a Gamatoto level)\nAn unlock of any other type, or one with no row, is always open";

pub const DROP_NOTICE: &str = "A special item an expedition to this area can bring back\nIt is rolled once for every hour of the expedition\nOnce the misses add up to the guaranteed hours, the next roll always hits";

pub const LIMIT_LABELS: [&str; 3] = ["Level Cap", "Destination Cap", "Helper Cap"];

pub const UNLOCK_LABELS: [&str; 4] = ["Unlock ID", "Needs", "User Rank", "Gamatoto Level"];

const UNLOCK_KIND: usize = 1;

const UNLOCK_KINDS: [Choice; 2] = [Choice::new(0, "Rank Or Level"), Choice::new(1, "Rank And Level")];

pub const DROP_LABELS: [&str; 7] = ["Drop ID", "Category", "Stage", "Item ID", "Chance Per Hour (of 10000)", "Quantity", "Guaranteed After (Hours)"];

const DROP_QUANTITY: usize = 5;

pub(super) fn limit_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn unlock_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        UNLOCK_KIND => Rule::Choice(&UNLOCK_KINDS),
        _ => Rule::Floor(0),
    }
}

pub(super) fn drop_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        DROP_QUANTITY => Rule::Floor(-1),
        _ => Rule::Floor(0),
    }
}

pub const AREA_NOTICE: &str = "One expedition area\nChances, weights and shares are out of 10000\nEach find roll picks one of five finds by weight, then an amount between its bounds\nA find's level weight is added once per point of Gamatoto's find bonus (in hundredths)\nA cap of 0 means no cap\nA recruit is an Assistant below 60 team points, and a Legend from 60 up";

pub const LEVEL_NOTICE: &str = "One Gamatoto level\nSets the total XP needed to reach it, the find bonus it gives, and the outfit he wears";

pub const AREA_LABELS: [&str; 45] = [
    "Plain Success Chance",
    "Recruit Chance",
    "Map Picture",
    "Find Rolls Per Hour",
    "Gamatoto XP",
    "Unlock ID",
    "Event Area ID",
    "Find Chance",
    "Find 1 Weight",
    "Find 1 Item ID",
    "Find 1 Min",
    "Find 1 Max",
    "Find 1 Weight Per Bonus",
    "Find 1 Cap",
    "Find 2 Weight",
    "Find 2 Item ID",
    "Find 2 Min",
    "Find 2 Max",
    "Find 2 Weight Per Bonus",
    "Find 2 Cap",
    "Find 3 Weight",
    "Find 3 Item ID",
    "Find 3 Min",
    "Find 3 Max",
    "Find 3 Weight Per Bonus",
    "Find 3 Cap",
    "Find 4 Weight",
    "Find 4 Item ID",
    "Find 4 Min",
    "Find 4 Max",
    "Find 4 Weight Per Bonus",
    "Find 4 Cap",
    "Find 5 Weight",
    "Find 5 Item ID",
    "Find 5 Min",
    "Find 5 Max",
    "Find 5 Weight Per Bonus",
    "Find 5 Cap",
    "Intern Share",
    "Lackey Share",
    "Underling Share",
    "Assistant Share",
    "Legend Share",
    "Engineer Share",
    "List Order",
];

pub const LEVEL_LABELS: [&str; 3] = ["Total XP", "Find Bonus", "Outfit"];

pub(super) const TEN_THOUSAND: i32 = 10000;

const FIRST_FIND: usize = 8;

const FIND_WIDTH: usize = 6;

const FIND_COUNT: usize = 5;

const FIRST_SHARE: usize = FIRST_FIND + FIND_WIDTH * FIND_COUNT;

const LIST_ORDER: usize = 44;

pub(super) const NO_ITEM: i32 = -1;

const LEVEL_WEIGHT: usize = 4;

pub(super) fn area_rule(index: usize) -> Rule {
    match index {
        0 | 1 | 7 => Rule::Range(0, TEN_THOUSAND),
        FIRST_SHARE..LIST_ORDER => Rule::Range(0, TEN_THOUSAND),
        FIRST_FIND..FIRST_SHARE => match (index - FIRST_FIND) % FIND_WIDTH {
            LEVEL_WEIGHT => Rule::Plain,
            _ => Rule::Floor(NO_ITEM),
        },
        _ => Rule::Floor(0),
    }
}

pub(super) fn level_rule() -> Rule {
    Rule::Floor(0)
}

pub const ZONE_NOTICE: &str = "Gamatoto's Zoned-In state\nSets how many minutes it lasts and how many expeditions it covers";

pub const ZONE_STEP_NOTICE: &str = "Expedition hours bank up while Gamatoto is not Zoned-In\nEach step the bank passes rolls its chance\nThe first hit empties the bank and starts Zoned-In";

pub const ZONE_LABELS: [&str; 2] = ["Minutes", "Expeditions"];

pub const ZONE_STEP_LABELS: [&str; 18] = [
    "Step 1 Hours",
    "Step 1 Chance (of 1000)",
    "Step 2 Hours",
    "Step 2 Chance (of 1000)",
    "Step 3 Hours",
    "Step 3 Chance (of 1000)",
    "Step 4 Hours",
    "Step 4 Chance (of 1000)",
    "Step 5 Hours",
    "Step 5 Chance (of 1000)",
    "Step 6 Hours",
    "Step 6 Chance (of 1000)",
    "Step 7 Hours",
    "Step 7 Chance (of 1000)",
    "Step 8 Hours",
    "Step 8 Chance (of 1000)",
    "Step 9 Hours",
    "Step 9 Chance (of 1000)",
];

pub(super) const THOUSAND: i32 = 1000;

pub(super) fn zone_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn zone_step_rule(index: usize) -> Rule {
    match index % 2 {
        1 => Rule::Range(0, THOUSAND),
        _ => Rule::Floor(0),
    }
}
