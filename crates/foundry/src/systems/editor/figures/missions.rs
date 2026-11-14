use super::resolved::Rule;

pub const NOTICE: &str = "A mission\nSets the row of the unlock table it waits on and the reward it pays out\nPopup When Achieved announces the mission with a popup once you achieve it\nMissions in the same state are listed by List Order, lowest first";

const POPUP: usize = 3;

pub const LABELS: [&str; 7] =
    ["Mission ID", "Unlock Condition ID", "List Order", "Popup When Achieved", "Reward Category", "Reward Item", "Reward Count"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        POPUP => Rule::Flag,
        _ => Rule::Floor(0),
    }
}

pub const UNLOCK_NOTICE: &str = "What a mission waits on before it shows up, and what locks it again\n-1 leaves a requirement unused\nA stage is (Map ID * 100) + Stage and must be cleared\nThe mission must be achieved\nAlso Needs Unlock is another row of this table that must be met too\nThe two day counts keep the mission open for that many days from their start";

pub const UNLOCK_LABELS: [&str; 11] = [
    "Unlock ID",
    "Cleared Stage",
    "Map Condition",
    "Cleared Mission",
    "User Rank",
    "Also Needs Unlock",
    "Challenge Days",
    "Invite Friends Days",
    "Locks At Stage",
    "Locks At Mission",
    "Locks At User Rank",
];

pub(super) fn unlock_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(-1),
    }
}
