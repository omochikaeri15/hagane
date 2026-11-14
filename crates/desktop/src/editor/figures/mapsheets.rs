use iced::Element;

use super::resolved::Rule;
use super::{Draft, Message};

const NOTICE: &str = "The pictures one map screen is built from\nSheet Set picks which numbered map images are loaded, and -1 uses the layout's own ID\nEach piece draws one cut of one sheet of that set at a position on the map\nPieces are read in order, and the list ends at the first piece with an empty sheet cell";

const NOTICE_SIZE: f32 = 11.0;

const HEAD: usize = 2;

const STRIDE: usize = 4;

const OWN_SET: i32 = -1;

pub(super) const LABELS: [&str; 34] = [
    "Map Layout ID", "Sheet Set",
    "Piece 1 Sheet", "Piece 1 X", "Piece 1 Y", "Piece 1 Cut",
    "Piece 2 Sheet", "Piece 2 X", "Piece 2 Y", "Piece 2 Cut",
    "Piece 3 Sheet", "Piece 3 X", "Piece 3 Y", "Piece 3 Cut",
    "Piece 4 Sheet", "Piece 4 X", "Piece 4 Y", "Piece 4 Cut",
    "Piece 5 Sheet", "Piece 5 X", "Piece 5 Y", "Piece 5 Cut",
    "Piece 6 Sheet", "Piece 6 X", "Piece 6 Y", "Piece 6 Cut",
    "Piece 7 Sheet", "Piece 7 X", "Piece 7 Y", "Piece 7 Cut",
    "Piece 8 Sheet", "Piece 8 X", "Piece 8 Y", "Piece 8 Cut",
];

pub(super) fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1 => Rule::Floor(OWN_SET),
        piece => match (piece - HEAD) % STRIDE {
            0 | 3 => Rule::Floor(0),
            _ => Rule::Plain,
        },
    }
}

pub(super) fn view<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    super::unitbuy::bounded(draft, width, query, armed, Some(NOTICE), NOTICE_SIZE, LABELS.len())
}
