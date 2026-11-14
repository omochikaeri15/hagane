use super::resolved::Rule;

pub const OFFERING_NOTICE: &str = "One offering at the shrine\nIts cost is paid in XP and also counts toward the shrine's level\nEach offering gives one gift, picked by weight";

pub const EXTRA_NOTICE: &str = "A bonus item an offering gives on top of its gift\nIt comes on every Nth offering of that kind, or only on exactly the Nth when Only Once is on";

pub const FORTUNE_NOTICE: &str = "One fortune rank\nThe three chances are out of 10000 and tried in order on a single roll, and whatever is left gives nothing\nThe first sends the shrine's servant to Gamatoto, and never hits while a Gamatoto collab helper is active\nThe second starts a two-day effect and needs User Rank 1600\nThe third gives the gift\nWhile the servant or the two-day effect is already running, all three chances count as 0";

pub const OFFERING_LABELS: [&str; 39] = [
    "Offering ID",
    "Cost (XP)",
    "Offerings Allowed",
    "Gift 1 Item ID",
    "Gift 1 Amount",
    "Gift 1 Weight",
    "Gift 2 Item ID",
    "Gift 2 Amount",
    "Gift 2 Weight",
    "Gift 3 Item ID",
    "Gift 3 Amount",
    "Gift 3 Weight",
    "Gift 4 Item ID",
    "Gift 4 Amount",
    "Gift 4 Weight",
    "Gift 5 Item ID",
    "Gift 5 Amount",
    "Gift 5 Weight",
    "Gift 6 Item ID",
    "Gift 6 Amount",
    "Gift 6 Weight",
    "Gift 7 Item ID",
    "Gift 7 Amount",
    "Gift 7 Weight",
    "Gift 8 Item ID",
    "Gift 8 Amount",
    "Gift 8 Weight",
    "Gift 9 Item ID",
    "Gift 9 Amount",
    "Gift 9 Weight",
    "Gift 10 Item ID",
    "Gift 10 Amount",
    "Gift 10 Weight",
    "Gift 11 Item ID",
    "Gift 11 Amount",
    "Gift 11 Weight",
    "Gift 12 Item ID",
    "Gift 12 Amount",
    "Gift 12 Weight",
];

pub const EXTRA_LABELS: [&str; 4] = ["Offering ID", "Only Once", "Offering Count", "Item ID"];

pub const FORTUNE_LABELS: [&str; 6] = ["Fortune Rank", "Servant Chance", "Two-Day Effect Chance", "Gift Chance", "Gift Item ID", "Gift Amount"];

pub(super) const TEN_THOUSAND: i32 = 10000;

const ONLY_ONCE: usize = 1;

const FIRST_CHANCE: usize = 1;

const LAST_CHANCE: usize = 3;

pub(super) fn offering_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub(super) fn extra_rule(index: usize) -> Rule {
    match index {
        ONLY_ONCE => Rule::Flag,
        _ => Rule::Floor(0),
    }
}

pub(super) fn fortune_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        FIRST_CHANCE..=LAST_CHANCE => Rule::Range(0, TEN_THOUSAND),
        _ => Rule::Floor(0),
    }
}
