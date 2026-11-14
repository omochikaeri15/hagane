use super::resolved::{Choice, Rule};

pub const SHORTCUT_NOTICE: &str = "Where a mission's Go button leads\nA Stage gathers the stages the mission's condition counts and sends you to one of them\nStage Pick decides if a cleared or uncleared stage wins, and if the map with the lower or higher Difficulty (Map Option table) wins\nThe game loads Filter Condition but never reads it";

pub const DESIGNATION_NOTICE: &str = "Ties a mission to a friend referral campaign\nAchieving the mission reports your referral as complete\n50000 is the standing campaign, and any other ID counts as a limited one";

pub const MONTHLY_NOTICE: &str = "A monthly mission set\nThe game keeps Missions At Once missions running and picks them at random from the Mission Group table\nIt takes Missions Per Group from the first group in the row, then moves on to the next group";

pub const SHORTCUT_LABELS: [&str; 4] = ["Shortcut ID", "Leads To", "Stage Pick", "Filter Condition"];

pub const DESIGNATION_LABELS: [&str; 2] = ["Mission ID", "Referral Campaign ID"];

pub const MONTHLY_LABELS: [&str; 15] = [
    "Monthly ID",
    "Missions At Once",
    "Missions Per Group",
    "Group 1",
    "Group 2",
    "Group 3",
    "Group 4",
    "Group 5",
    "Group 6",
    "Group 7",
    "Group 8",
    "Group 9",
    "Group 10",
    "Group 11",
    "Group 12",
];

const QUANTITY: usize = 1;

const NUMBER: usize = 2;

const DESTINATION: usize = 1;

const PICK: usize = 2;

const DESTINATIONS: [Choice; 3] = [Choice::new(0, "Nowhere"), Choice::new(1, "A Stage"), Choice::new(2, "Gamatoto")];

const PICKS: [Choice; 5] = [
    Choice::new(-1, "None"),
    Choice::new(0, "Cleared, Lower Difficulty"),
    Choice::new(1, "Cleared, Higher Difficulty"),
    Choice::new(2, "Uncleared, Lower Difficulty"),
    Choice::new(3, "Uncleared, Higher Difficulty"),
];

pub(super) fn shortcut_rule(index: usize) -> Rule {
    match index {
        DESTINATION => Rule::Choice(&DESTINATIONS),
        PICK => Rule::Choice(&PICKS),
        _ => Rule::Opaque,
    }
}

pub(super) fn designation_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(-1),
    }
}

pub(super) fn monthly_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        QUANTITY | NUMBER => Rule::Floor(0),
        _ => Rule::Floor(-1),
    }
}

pub(super) fn monthly_fallback(index: usize) -> i32 {
    match index {
        0..=NUMBER => 0,
        _ => -1,
    }
}
