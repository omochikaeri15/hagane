use iced::Element;

use foundry::domains::sandbox::base;

use super::resolved::{Choice, Rule};
use super::{Draft, Message};

const NOTICE: &str = "One row per cannon part\nBehavior picks how the cannon fires\nThe unit is the wall or target the behavior uses";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 7] = ["Part ID", "Behavior", "Makes Wave", "Unit ID", "Knocks Back", "Ready Effect", "Soulstrike"];

const BEHAVIORS: &[Choice] = &[
    Choice::new(0, base::CANNONS[0]),
    Choice::new(1, base::CANNONS[1]),
    Choice::new(2, base::CANNONS[2]),
    Choice::new(3, base::CANNONS[3]),
    Choice::new(4, base::CANNONS[4]),
    Choice::new(5, base::CANNONS[5]),
    Choice::new(6, base::CANNONS[6]),
    Choice::new(7, base::CANNONS[7]),
];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1 => Rule::Choice(BEHAVIORS),
        2 | 4 | 5 | 6 => Rule::Flag,
        _ => Rule::Plain,
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
