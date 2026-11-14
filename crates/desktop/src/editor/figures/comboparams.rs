use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per combo effect\nEach column is how strong the effect is at that combo size (in the effect's own unit)\nDown is what a weakening combo applies";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const BANDS: [&str; 6] = ["Small", "Medium", "Large", "Ultimate", "Down", "Granted"];

pub(super) fn rule() -> Rule {
    Rule::Plain
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    let held = draft.stored.clamp(1, BANDS.len());

    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, held)
}
