use super::resolved::Rule;

pub const NOTICE: &str = "One row per screen shake, found by its ID\nShake 1 plays when a boss shockwave lands, shake 2 plays when a surge erupts\nThe strength eases from its start to its end over the frames (0 frames counts as 1)";

pub const LABELS: [&str; 7] =
    ["Shake ID", "Start Strength", "End Strength", "Frames", "Count Until Next", "Reset Frame", "Priority"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1 | 2 => Rule::Plain,
        3 => Rule::Floor(1),
        _ => Rule::Floor(0),
    }
}
