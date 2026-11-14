use super::resolved::Rule;

pub const NOTICE: &str = "Clearing this stage raises the altar enemy's level cap by this amount, and can unseal its altar\nThe row is found by (Map ID * 100) + Stage";

pub const LABELS: [&str; 4] = ["Stage Key", "Level Cap Raise", "Unseals Altar", "Altar Enemy ID"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        2 => Rule::Flag,
        _ => Rule::Floor(0),
    }
}
