use iced::Element;

use super::resolved::Rule;
use super::schema::BRACKET;
use super::{Draft, Message};

const NOTICE: &str = "One value per ten-level bracket, in the same brackets as this cat's level curve\nThey set this cat's level-up prices in XP";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const BRACKETS: usize = 20;

pub(super) fn label(index: usize) -> String {
    format!("Levels {}-{}", index * BRACKET + 1, (index + 1) * BRACKET)
}

pub(super) fn rule() -> Rule {
    Rule::Floor(0)
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, BRACKETS)
}
