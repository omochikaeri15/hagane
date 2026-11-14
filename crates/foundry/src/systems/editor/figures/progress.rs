use super::resolved::Rule;

pub const STAMP_NOTICE: &str = "One row per day of the login stamp card, in day order\nSets what kind of reward that day gives and how much";

pub const SLOT_NOTICE: &str = "One row per lineup slot, in slot order\n-1 leaves the slot without a price";

pub const STAMP_LABELS: [&str; 2] = ["Reward Type", "Amount"];

pub const SLOT_LABELS: [&str; 2] = ["Unlock Option", "Quantity"];

pub(super) fn stamp_rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn slot_rule() -> Rule {
    Rule::Floor(-1)
}
