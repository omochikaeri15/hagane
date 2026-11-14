use super::resolved::Rule;

pub const NOTICE: &str = "One price bracket of the Super CPU\nUses count from 1, and the bracket holding the next use sets how many Cat CPU items it costs\nIt uses the plain cost, or the pass cost while an Officer's Club pass is active\nThe highest last use across all brackets is the limit\nA use that no bracket holds costs 9999";

pub const LABELS: [&str; 4] = ["First Use", "Last Use", "Cost", "Cost With Pass"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Floor(1),
        _ => Rule::Floor(0),
    }
}
