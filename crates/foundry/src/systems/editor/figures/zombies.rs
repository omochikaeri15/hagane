use super::resolved::Rule;

pub const HEADER_NOTICE: &str = "The lottery's first line\nEvery interval, the game rolls each tier you have reached";

pub const TIER_NOTICE: &str = "A tier applies once the map has this many cleared zombie stages\nEach interval it rolls this many draws at this chance, and always gives at least the minimum";

pub const HEADER_LABELS: [&str; 2] = ["Lottery ID", "Interval (Minutes)"];

pub const TIER_LABELS: [&str; 4] = ["Cleared Zombie Stages", "Draws", "Chance %", "Minimum Granted"];

pub(super) const CHANCE: usize = 2;

pub(super) const PERCENT: i32 = 100;

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
