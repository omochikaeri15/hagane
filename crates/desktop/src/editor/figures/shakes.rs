use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per screen shake, found by its ID\nShake 1 plays when a boss shockwave lands, shake 2 plays when a surge erupts\nThe strength eases from its start to its end over the frames (0 frames counts as 1)";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 7] =
    ["Shake ID", "Start Strength", "End Strength", "Frames", "Count Until Next", "Reset Frame", "Priority"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1 | 2 => Rule::Plain,
        3 => Rule::Floor(1),
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
