use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per EX lottery group, in group order\nA stage's EX draws use these groups by their position";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 2] = ["EX Map ID", "EX Stage"];

pub(super) fn rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
