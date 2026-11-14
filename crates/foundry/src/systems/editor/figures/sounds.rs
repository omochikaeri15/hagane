use super::resolved::Rule;

pub const SOUND_NOTICE: &str = "How the game plays one sound (music or effect, if it loops, and its priority)\nThis table decides looping, not the audio file";

pub const CHEST_NOTICE: &str = "One treasure chest\nSets the ID it is found by, its cost, and the rewards it holds\nThe game shuffles the rewards every time it reads them";

pub const SOUND_LABELS: [&str; 3] = ["Music", "Loops", "Priority"];

pub const CHEST_LABELS: [&str; 9] = ["Chest Item ID", "Cost", "Drop 1", "Drop 2", "Drop 3", "Drop 4", "Drop 5", "Drop 6", "Drop 7"];

const PRIORITY: usize = 2;

const COST: usize = 1;

pub(super) fn sound_rule(index: usize) -> Rule {
    match index {
        PRIORITY => Rule::Floor(-1),
        _ => Rule::Flag,
    }
}

pub(super) fn chest_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        COST => Rule::Floor(0),
        _ => Rule::Floor(-1),
    }
}

pub(super) fn chest_fallback(index: usize) -> i32 {
    match index {
        0 | COST => 0,
        _ => -1,
    }
}
