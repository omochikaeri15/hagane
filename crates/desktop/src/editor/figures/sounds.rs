use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const SOUND_NOTICE: &str = "How the game plays one sound (music or effect, if it loops, and its priority)\nThis table decides looping, not the audio file";
const CHEST_NOTICE: &str = "One treasure chest\nSets the ID it is found by, its cost, and the rewards it holds\nThe game shuffles the rewards every time it reads them";

const NOTICE_SIZE: f32 = 11.0;

pub(super) const SOUND_LABELS: [&str; 3] = ["Music", "Loops", "Priority"];
pub(super) const CHEST_LABELS: [&str; 9] = ["Chest Item ID", "Cost", "Drop 1", "Drop 2", "Drop 3", "Drop 4", "Drop 5", "Drop 6", "Drop 7"];

const PRIORITY: usize = 2;
const COST: usize = 1;

pub(super) fn sound_rule(index: usize) -> Rule {
    match index {
        PRIORITY => Rule::Floor(-1),
        _ => Rule::Flag,
    }
}

pub(super) fn chest_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        COST => Rule::Floor(0),
        _ => Rule::Floor(-1),
    }
}

pub(super) fn chest_fallback(index: usize) -> i32 {
    match index {
        0 | COST => 0,
        _ => -1,
    }
}

pub(super) fn sound_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(SOUND_NOTICE), NOTICE_SIZE, SOUND_LABELS.len())
}

pub(super) fn chest_view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(CHEST_NOTICE), NOTICE_SIZE, CHEST_LABELS.len())
}
