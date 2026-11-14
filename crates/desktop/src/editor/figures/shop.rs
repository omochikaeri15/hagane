use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per item shop slot\nThe price is in Cat Food\nThe category is the text key the shop tab shows";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 7] =
    ["Shop Slot", "Item ID", "Quantity", "Price (Cat Food)", "Shows Quantity", "Category Text Key", "Icon"];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 | 5 => Rule::Opaque,
        4 => Rule::Flag,
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
