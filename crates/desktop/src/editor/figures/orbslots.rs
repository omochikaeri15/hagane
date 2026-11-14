use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "How many Talent Orbs this cat can equip\nSetting the slots to 0 removes the cat from the table (every shipped cat has 1 or 2)\nA slot with its last cell on stays locked until the cat reaches level 60";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const LABELS: [&str; 4] = ["Cat ID", "Orb Slots", "Slot 1 Needs Level 60", "Slot 2 Needs Level 60"];

pub(super) const SLOTS: usize = 1;
pub(super) const SEEDED: usize = 2;

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        SLOTS => Rule::Floor(0),
        _ => Rule::Flag,
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
