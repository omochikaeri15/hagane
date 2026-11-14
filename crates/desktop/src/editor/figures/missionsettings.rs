use iced::Element;

use super::resolved::{Choice, Rule};
use super::{Draft, Message};

const SHORTCUT_NOTICE: &str = "Where a mission's Go button leads\nA Stage gathers the stages the mission's condition counts and sends you to one of them\nStage Pick decides if a cleared or uncleared stage wins, and if the map with the lower or higher Difficulty (Map Option table) wins\nThe game loads Filter Condition but never reads it";
const DESIGNATION_NOTICE: &str = "Ties a mission to a friend referral campaign\nAchieving the mission reports your referral as complete\n50000 is the standing campaign, and any other ID counts as a limited one";
const MONTHLY_NOTICE: &str = "A monthly mission set\nThe game keeps Missions At Once missions running and picks them at random from the Mission Group table\nIt takes Missions Per Group from the first group in the row, then moves on to the next group";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const SHORTCUT_LABELS: [&str; 4] = ["Shortcut ID", "Leads To", "Stage Pick", "Filter Condition"];
pub(super) const DESIGNATION_LABELS: [&str; 2] = ["Mission ID", "Referral Campaign ID"];
pub(super) const MONTHLY_LABELS: [&str; 15] = [
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

pub(super) fn shortcut_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SHORTCUT_NOTICE), NOTICE_SIZE, SHORTCUT_LABELS.len())
}

pub(super) fn designation_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(DESIGNATION_NOTICE), NOTICE_SIZE, DESIGNATION_LABELS.len())
}

pub(super) fn monthly_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(MONTHLY_NOTICE), NOTICE_SIZE, MONTHLY_LABELS.len())
}
