use super::resolved::Rule;

pub const NOTICE: &str = "One row per map, found by its global Map ID, then one difficulty per stage in stage order\nType into the empty cell after the last stage to rate a stage the row doesn't reach yet";

pub(super) const STAGES: usize = 50;

const PLACES: u32 = 1;

pub const WIDTH: usize = STAGES + 1;

const KEY_LABEL: &str = "Map ID";

pub(super) fn label(index: usize) -> String {
    match index.checked_sub(1) {
        Some(stage) => format!("Stage {stage:02}"),
        None => KEY_LABEL.to_owned(),
    }
}

pub(super) fn decimals(index: usize) -> u32 {
    match index {
        0 => 0,
        _ => PLACES,
    }
}

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}
