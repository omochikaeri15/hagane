use super::resolved::Rule;

pub const NOTICE: &str = "What a Labyrinth floor rolls when you leave it, one rule after a clear and another after a defeat\nEach one is a row of the floor lottery";

pub const LOTTERY_NOTICE: &str = "A floor lottery rule\nThe random exit count is picked between its bounds, then one avoidable count by weight out of 100";

pub const LABELS: [&str; 2] = ["Rule On Clear", "Rule On Defeat"];

pub const LOTTERY_LABELS: [&str; 11] = [
    "Rule ID",
    "Random Exits Min",
    "Random Exits Max",
    "Avoidable Count 1",
    "Weight 1 (%)",
    "Avoidable Count 2",
    "Weight 2 (%)",
    "Avoidable Count 3",
    "Weight 3 (%)",
    "Avoidable Count 4",
    "Weight 4 (%)",
];

const FIRST_OUTCOME: usize = 3;

pub(super) const PERCENT: i32 = 100;

pub fn rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn lottery_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ if index >= FIRST_OUTCOME && (index - FIRST_OUTCOME) % 2 == 1 => Rule::Range(0, PERCENT),
        _ => Rule::Floor(0),
    }
}
