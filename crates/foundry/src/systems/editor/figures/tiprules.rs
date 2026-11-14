use super::resolved::{Choice, Rule};

pub const NOTICE: &str = "Where a defeat tip can show\nA tip is hidden wherever its flag is off\nA target type limits it to the listed stages or maps, read until the first -1";

pub const LABELS: [&str; 40] = [
    "Tip ID",
    "Empire of Cats Ch. 1",
    "Empire of Cats Ch. 1 Outbreak",
    "Empire of Cats Ch. 2",
    "Empire of Cats Ch. 2 Outbreak",
    "Empire of Cats Ch. 3",
    "Empire of Cats Ch. 3 Outbreak",
    "Aku Realm",
    "Into the Future Ch. 1",
    "Into the Future Ch. 1 Outbreak",
    "Into the Future Ch. 2",
    "Into the Future Ch. 2 Outbreak",
    "Into the Future Ch. 3",
    "Into the Future Ch. 3 Outbreak",
    "Cats of the Cosmos Ch. 1",
    "Cats of the Cosmos Ch. 1 Outbreak",
    "Cats of the Cosmos Ch. 2",
    "Cats of the Cosmos Ch. 2 Outbreak",
    "Cats of the Cosmos Ch. 3",
    "Cats of the Cosmos Ch. 3 Outbreak",
    "Cats of the Cosmos Ch. 3 Invasion",
    "Other Maps, 1 Crown",
    "Other Maps, 2 Crowns",
    "Other Maps, 3 Crowns",
    "Other Maps, 4 Crowns",
    "Other Maps, 5 Crowns",
    "EX Stages",
    "Target Type",
    "Target 1",
    "Target 2",
    "Target 3",
    "Target 4",
    "Target 5",
    "Target 6",
    "Target 7",
    "Target 8",
    "Target 9",
    "Target 10",
    "Target 11",
    "Target 12",
];

const LAST_FLAG: usize = 26;

const TARGET_TYPE: usize = 27;

const TARGET_TYPES: [Choice; 3] = [Choice::new(-1, "None"), Choice::new(1, "Stages (Map x 100 + Stage)"), Choice::new(2, "Maps")];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1..=LAST_FLAG => Rule::Flag,
        TARGET_TYPE => Rule::Choice(&TARGET_TYPES),
        _ => Rule::Floor(-1),
    }
}

pub(super) fn fallback(index: usize) -> i32 {
    match index {
        0..=LAST_FLAG => 0,
        _ => -1,
    }
}
