pub(super) const TITLE: &str = "Unused File";

pub(super) const FOLDER: &str = "Unused";

pub const FILES: [(&str, Fate); 4] = [
    ("AbilityLimit.csv", Fate::Unread),
    ("point_condition.csv", Fate::Unread),
    ("rewardStage.csv", Fate::Unheeded),
    ("equipment_filter.tsv", Fate::Unheeded),
];

pub(super) const FAMILIES: [(&str, &str, &str); 1] = [("gatyachara_", "_z.png", "Black Gacha Images")];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    Unread,
    Unheeded,
}
