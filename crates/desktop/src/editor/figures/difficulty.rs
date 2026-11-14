use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "One row per map, found by its global Map ID, then one difficulty per stage in stage order\nType into the empty cell after the last stage to rate a stage the row doesn't reach yet";

const NOTICE_SIZE: f32 = 11.0;

const STAGES: usize = 50;
const PLACES: u32 = 1;

pub(super) const WIDTH: usize = STAGES + 1;
pub(super) const KEY_LABEL: &str = "Map ID";

pub(super) fn label(index: usize) -> String {
    match index.checked_sub(1) {
        Some(stage) => format!("Stage {stage:02}"),
        None => KEY_LABEL.to_owned(),
    }
}

pub(super) fn decimals(index: usize) -> u32 {
    match index {
        0 => 0,
        _ => PLACES,
    }
}

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    let reach = (draft.stored + 1).min(WIDTH);

    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, reach)
}
