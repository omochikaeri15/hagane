use super::resolved::Rule;

pub const NOTICE: &str = "Where an enemy castle sits and how big it draws\nAn Offset X of -999 ends the table, so keep it above that";

pub const LABELS: [&str; 4] = ["Offset X", "Offset Y", "Size (%)", "Art Variant"];

const OFFSET_X: usize = 0;

const OFFSET_Y: usize = 1;

const LAST_ROW: i32 = -998;

pub fn rule(index: usize) -> Rule {
    match index {
        OFFSET_X => Rule::Floor(LAST_ROW),
        OFFSET_Y => Rule::Plain,
        _ => Rule::Floor(0),
    }
}
