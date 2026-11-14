use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "An Officers' Club reward\nThe item count is how many of the item triplets after it are read";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 13] = [
    "Club Item ID",
    "Lower Limit",
    "Upper Limit",
    "Item Count",
    "Item 1 Category",
    "Item 1 ID",
    "Item 1 Value",
    "Item 2 Category",
    "Item 2 ID",
    "Item 2 Value",
    "Item 3 Category",
    "Item 3 ID",
    "Item 3 Value",
];

const ITEM_COUNT: usize = 3;
const MAX_ITEMS: i32 = 3;

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        ITEM_COUNT => Rule::Range(0, MAX_ITEMS),
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
