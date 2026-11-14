use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "Where an enemy castle sits and how big it draws\nAn Offset X of -999 ends the table, so keep it above that";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 4] = ["Offset X", "Offset Y", "Size (%)", "Art Variant"];

const OFFSET_X: usize = 0;
const OFFSET_Y: usize = 1;
const LAST_ROW: i32 = -998;

pub(super) fn rule(index: usize) -> Rule {
    match index {
        OFFSET_X => Rule::Floor(LAST_ROW),
        OFFSET_Y => Rule::Plain,
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
