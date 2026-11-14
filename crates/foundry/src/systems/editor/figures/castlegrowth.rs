use super::resolved::{Choice, Rule};

pub const NOTICE: &str = "One bracket of the Cat Base's health curve\nFrom the level the previous bracket ends at up to this one, health goes from From to To along the easing";

pub const LABELS: [&str; 4] = ["Up To Level", "Base Health From", "Base Health To", "Easing"];

pub(super) const LEVEL: usize = 0;

const EASING: usize = 3;

const EASINGS: [Choice; 3] = [Choice::new(0, "Linear"), Choice::new(1, "Cubic Ease In"), Choice::new(2, "Cubic Ease Out")];

pub fn rule(index: usize) -> Rule {
    match index {
        LEVEL => Rule::Floor(1),
        EASING => Rule::Choice(&EASINGS),
        _ => Rule::Floor(0),
    }
}
