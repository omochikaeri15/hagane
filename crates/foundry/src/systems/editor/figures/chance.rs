use crate::systems::editor::roster::CAPSULES as CAPSULE_NAMES;
use super::resolved::{Choice, Rule};

pub const NOTICE: &str = "Which special animation a capsule roll can play\nEach slot is a rarity, its weight and the animation\nThe game only reads whole slots";

pub const LABELS: [&str; 17] = [
    "Capsule Type",
    "Series ID",
    "Slot 1 Rarity",
    "Slot 1 Weight",
    "Slot 1 Animation",
    "Slot 2 Rarity",
    "Slot 2 Weight",
    "Slot 2 Animation",
    "Slot 3 Rarity",
    "Slot 3 Weight",
    "Slot 3 Animation",
    "Slot 4 Rarity",
    "Slot 4 Weight",
    "Slot 4 Animation",
    "Slot 5 Rarity",
    "Slot 5 Weight",
    "Slot 5 Animation",
];

pub(super) const CAPSULES: [Choice; 3] = [Choice::new(0, CAPSULE_NAMES[0].1), Choice::new(1, CAPSULE_NAMES[1].1), Choice::new(2, CAPSULE_NAMES[2].1)];

pub(super) const RARITIES: [Choice; 6] = [
    Choice::new(0, "Normal"),
    Choice::new(1, "Special"),
    Choice::new(2, "Rare"),
    Choice::new(3, "Super Rare"),
    Choice::new(4, "Uber Super Rare"),
    Choice::new(5, "Legend Rare"),
];

pub(super) const HEAD: usize = 2;

pub(super) const STRIDE: usize = 3;

const RARITY: usize = 0;

const WEIGHT: usize = 1;

pub fn rule(index: usize) -> Rule {
    match index.checked_sub(HEAD).map(|at| at % STRIDE) {
        None if index == 0 => Rule::Choice(&CAPSULES),
        None => Rule::Floor(-1),
        Some(RARITY) => Rule::Choice(&RARITIES),
        Some(WEIGHT) => Rule::Floor(0),
        Some(_) => Rule::Floor(-1),
    }
}

pub(super) fn fallback(index: usize) -> i32 {
    match index.checked_sub(HEAD).map(|at| at % STRIDE) {
        None if index == 0 => 0,
        Some(RARITY | WEIGHT) => 0,
        _ => -1,
    }
}
