use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per orb grade, from D up to S\nThe last grade takes -1 orbs to upgrade since nothing is above it";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 4] = ["Orbs to Upgrade", "NP When Sold", "NP to Remove", "Grade Letter"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Floor(-1),
        1 | 2 => Rule::Floor(0),
        _ => Rule::Opaque,
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
