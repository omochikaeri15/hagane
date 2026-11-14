use iced::Element;

use crate::editor::roster::CAPSULES as CAPSULE_NAMES;

use super::resolved::{Choice, Rule};
use super::{Draft, Message};

const NOTICE: &str = "Which special animation a capsule roll can play\nEach slot is a rarity, its weight and the animation\nThe game only reads whole slots";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 17] = [
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

const CAPSULES: [Choice; 3] = [Choice::new(0, CAPSULE_NAMES[0].1), Choice::new(1, CAPSULE_NAMES[1].1), Choice::new(2, CAPSULE_NAMES[2].1)];

const RARITIES: [Choice; 6] = [
    Choice::new(0, "Normal"),
    Choice::new(1, "Special"),
    Choice::new(2, "Rare"),
    Choice::new(3, "Super Rare"),
    Choice::new(4, "Uber Super Rare"),
    Choice::new(5, "Legend Rare"),
];

const HEAD: usize = 2;
const STRIDE: usize = 3;
const RARITY: usize = 0;
const WEIGHT: usize = 1;

pub(super) fn rule(index: usize) -> Rule {
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

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
