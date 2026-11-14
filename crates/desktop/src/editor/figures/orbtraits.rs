use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "The enemy traits an orb with this trait works against\nAn orb can cover more than one";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 13] = [
    "Red", "Floating", "Black", "Metal", "Angel", "Alien", "Zombie", "Relic", "Traitless", "Witch", "Eva Angel", "Aku",
    "No Trait",
];

pub(super) fn rule() -> Rule {
    Rule::Flag
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
