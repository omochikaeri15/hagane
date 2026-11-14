use super::resolved::Rule;

pub const NOTICE: &str = "Clearing this map opens the special EX map named here\nThe game stops reading this file at its first comment line, so only rows above the comments can be changed";

pub const LABELS: [&str; 2] = ["Map ID", "EX Map ID"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}
