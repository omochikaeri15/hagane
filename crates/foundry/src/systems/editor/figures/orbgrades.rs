use super::resolved::Rule;

pub const NOTICE: &str = "One row per orb grade, from D up to S\nThe last grade takes -1 orbs to upgrade since nothing is above it";

pub const LABELS: [&str; 4] = ["Orbs to Upgrade", "NP When Sold", "NP to Remove", "Grade Letter"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Floor(-1),
        1 | 2 => Rule::Floor(0),
        _ => Rule::Opaque,
    }
}
