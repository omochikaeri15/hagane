use super::resolved::Rule;

pub const NOTICE: &str = "When an unlock popup fires and what it hands out\nThe game only shows it when Enabled is exactly 1, and never on Android when OS ID is 1\nA stage is (Map ID * 100) + Stage and must be cleared when Conditions is 0 (-1 means no stage)\nMap Conditions is an unlock group that must be met (-1 means none), and the User Rank must be reached\nThe hand-outs are two cats, an item with its count, and level cap raises\nTalents Gated is (Unit ID * 10), and that unit's talents stay locked on the upgrade screen until every popup naming it is granted\nDraws Help Picture makes the popup draw its help picture";

pub const LABELS: [&str; 19] = [
    "Popup ID",
    "Enabled",
    "Conditions",
    "Stage",
    "Map Conditions",
    "User Rank",
    "Cat Granted 1",
    "Cat Granted 2",
    "OS ID",
    "Unlock Eye 1 ID",
    "Add Level 1",
    "Unlock Eye 2 ID",
    "Add Level 2",
    "Unlock Plus ID",
    "Add Plus Level",
    "Talents Gated (Unit ID x10)",
    "Item ID",
    "Item Count",
    "Draws Help Picture",
];

const ENABLED: usize = 1;

pub(super) const STAGE: usize = 3;

const RANK: usize = 5;

const OS: usize = 8;

const HELP: usize = 18;

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        ENABLED | HELP => Rule::Flag,
        STAGE | OS => Rule::Plain,
        RANK => Rule::Floor(0),
        _ => Rule::Floor(-1),
    }
}
