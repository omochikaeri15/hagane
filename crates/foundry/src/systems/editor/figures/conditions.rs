use super::resolved::{Choice, Rule};

pub const NOTICE: &str = "What a mission asks for\nMission Type is the tab it is listed under\nShortcut ID is a row of the Mission Shortcut table\nThe values are the IDs the condition counts, and -1 means any\nA stage is (Map ID * 100) + Stage, a map type is Map ID / 1000, and crowns count from 0\nProgress Needed is how many times, or the score to reach for the two score conditions\nScore In Ranking Stage uses (Ranking * 1000) + (Stage * 10) + Crown\nClear Stage At Crown uses (Map ID * 1000) + (Stage * 10) + Crown\nClear Colosseum Stage uses (Stage * 10) + Crown\nDefeat Enemy reads Stage, Crown and Enemy ID, then the Map IDs it counts in\nClear Guerrilla Schedule Stages takes a bit number of the Guerrilla Schedule cell in the Map Option table\nCollect Treasures uses (Chapter * 100) + Stage\nType into the empty cell after the last value to add one";

pub const HEAD: [&str; 5] = ["Mission ID", "Mission Type", "Condition Type", "Shortcut ID", "Progress Needed"];

pub(super) const VALUES: usize = 64;

pub const WIDTH: usize = HEAD.len() + VALUES;

pub(super) fn label(index: usize) -> String {
    match HEAD.get(index) {
        Some(label) => (*label).to_owned(),
        None => format!("Value {}", index - HEAD.len() + 1),
    }
}

const MISSION_TYPE: usize = 1;

const CONDITION_TYPE: usize = 2;

const MISSION_TYPES: [Choice; 4] = [Choice::new(0, "Regular"), Choice::new(1, "Weekly"), Choice::new(2, "Special"), Choice::new(3, "Monthly")];

const CONDITION_TYPES: [Choice; 35] = [
    Choice::new(0, "Clear Any Stage Of Maps"),
    Choice::new(1, "Clear Stages"),
    Choice::new(2, "Own Units"),
    Choice::new(3, "Finish Stories Of Legend At Crown"),
    Choice::new(4, "Own Units Of Rarity"),
    Choice::new(5, "Unlock True Form Of Rarity"),
    Choice::new(6, "Achieve Missions"),
    Choice::new(7, "Finish Gamatoto Expeditions"),
    Choice::new(8, "Raise Unit Level"),
    Choice::new(9, "Clear Any Stage Of Map Types"),
    Choice::new(10, "Draw Capsules"),
    Choice::new(11, "Collect Treasures"),
    Choice::new(12, "Open Cat Guide"),
    Choice::new(13, "Open Enemy Guide"),
    Choice::new(14, "Open Item Shop"),
    Choice::new(15, "Clear With Cat Combo"),
    Choice::new(16, "Recruit Gamatoto Assistant"),
    Choice::new(17, "Raise User Rank"),
    Choice::new(18, "Score In Ranking Stage"),
    Choice::new(19, "Share From Options"),
    Choice::new(20, "Clear Guerrilla Schedule Stages"),
    Choice::new(21, "Score In Stage"),
    Choice::new(22, "Defeat Enemy"),
    Choice::new(23, "Achieve Monthly Missions"),
    Choice::new(24, "Clear Under Mission Limits"),
    Choice::new(25, "Collect Superior Treasures Of Map"),
    Choice::new(26, "Draw Units From Capsules"),
    Choice::new(27, "Cat Base Talk"),
    Choice::new(28, "First Clears In Map Types"),
    Choice::new(29, "Finish Uncanny Legends At Crown"),
    Choice::new(30, "Clear Colosseum Stage"),
    Choice::new(31, "Link Account"),
    Choice::new(32, "Draw Featured Capsule Item"),
    Choice::new(33, "Tutorial Action"),
    Choice::new(34, "Clear Stage At Crown"),
];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        MISSION_TYPE => Rule::Choice(&MISSION_TYPES),
        CONDITION_TYPE => Rule::Choice(&CONDITION_TYPES),
        _ if index < HEAD.len() => Rule::Floor(0),
        _ => Rule::Plain,
    }
}

pub const GROUP_NOTICE: &str = "The missions shown together under this group\nType into the empty cell after the last mission to add one";

const GROUP_MISSIONS: usize = 64;

pub const GROUP_WIDTH: usize = 1 + GROUP_MISSIONS;

pub(super) fn group_label(index: usize) -> String {
    match index {
        0 => "Group ID".to_owned(),
        _ => format!("Mission {index}"),
    }
}

pub(super) fn group_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}
