use iced::Element;

use super::resolved::{Choice, Rule};
use super::{Draft, Message};

const NOTICE: &str = "One bracket of the Cat Base's health curve\nFrom the level the previous bracket ends at up to this one, health goes from From to To along the easing";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 4] = ["Up To Level", "Base Health From", "Base Health To", "Easing"];

const LEVEL: usize = 0;
const EASING: usize = 3;

const EASINGS: [Choice; 3] = [Choice::new(0, "Linear"), Choice::new(1, "Cubic Ease In"), Choice::new(2, "Cubic Ease Out")];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        LEVEL => Rule::Floor(1),
        EASING => Rule::Choice(&EASINGS),
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
