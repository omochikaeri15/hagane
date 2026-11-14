use crate::domains::sandbox::base;
use super::resolved::{Choice, Rule};

pub const NOTICE: &str = "One row per cannon part\nBehavior picks how the cannon fires\nThe unit is the wall or target the behavior uses";

pub const LABELS: [&str; 7] = ["Part ID", "Behavior", "Makes Wave", "Unit ID", "Knocks Back", "Ready Effect", "Soulstrike"];

const BEHAVIORS: &[Choice] = &[
    Choice::new(0, base::CANNONS[0]),
    Choice::new(1, base::CANNONS[1]),
    Choice::new(2, base::CANNONS[2]),
    Choice::new(3, base::CANNONS[3]),
    Choice::new(4, base::CANNONS[4]),
    Choice::new(5, base::CANNONS[5]),
    Choice::new(6, base::CANNONS[6]),
    Choice::new(7, base::CANNONS[7]),
];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1 => Rule::Choice(BEHAVIORS),
        2 | 4 | 5 | 6 => Rule::Flag,
        _ => Rule::Plain,
    }
}
