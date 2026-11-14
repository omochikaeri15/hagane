use super::resolved::Rule;

pub const PATTERN_NOTICE: &str = "One prize of a slot machine or scratch card\nRows sharing an attempt number are picked against each other by weight\nA prize category of 0 is an item (the only kind the shipped tables use)\nThe animation is the number of the machine animation that plays for this prize\nThe share frame is the frame drawn while the result is captured for sharing";

pub const PATTERN_LABELS: [&str; 7] = ["Attempt", "Weight", "Prize Category", "Prize ID", "Amount", "Animation", "Share Frame"];

const NO_FRAME: i32 = -1;

const SHARE_FRAME: usize = 6;

pub(super) fn pattern_rule(index: usize) -> Rule {
    match index {
        SHARE_FRAME => Rule::Floor(NO_FRAME),
        _ => Rule::Floor(0),
    }
}

pub const REEL_NOTICE: &str = "One symbol of a slot machine and the picture it uses on each of the three reels\nThe rows are the order of the reel strip\nA reel that stops on a symbol shows the rows before and after it as its neighbors";

pub const REEL_LABELS: [&str; 3] = ["Reel 1 Picture", "Reel 2 Picture", "Reel 3 Picture"];

pub(super) fn reel_rule() -> Rule {
    Rule::Floor(0)
}
