use super::resolved::Rule;

pub const NOTICE: &str = "A category in the Cat Combo filter and the combo effect types it holds, read to the end of the row";

pub const LABELS: [&str; 13] = [
    "Category ID",
    "Effect Type 1",
    "Effect Type 2",
    "Effect Type 3",
    "Effect Type 4",
    "Effect Type 5",
    "Effect Type 6",
    "Effect Type 7",
    "Effect Type 8",
    "Effect Type 9",
    "Effect Type 10",
    "Effect Type 11",
    "Effect Type 12",
];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}
