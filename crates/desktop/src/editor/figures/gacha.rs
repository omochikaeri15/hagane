use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "How a capsule set looks (its banner, ticket, animation and images)\nA short row reads its missing columns as unset";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 11] = [
    "Set ID",
    "Banner Shown",
    "Ticket Item ID",
    "Capsule Animation",
    "Button Image",
    "Series ID",
    "Menu Image",
    "Featured Cat",
    "Idle Animation",
    "Image ID",
    "Get Count",
];

const BANNER: usize = 1;
const SERIES: usize = 5;
const IDLE: usize = 8;
const COUNT: usize = 10;

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        BANNER | IDLE => Rule::Flag,
        SERIES | COUNT => Rule::Floor(0),
        _ => Rule::Floor(-1),
    }
}

pub(super) fn fallback(index: usize) -> i32 {
    match index {
        0 | BANNER | SERIES | IDLE | COUNT => 0,
        _ => -1,
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
