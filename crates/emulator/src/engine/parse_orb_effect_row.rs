use super::{AssetStream, read_csv_cell};

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct OrbEffectRow {
    pub id: i32,
    pub kind: i32,
    pub serial: i32,
    pub targets: [i32; 3],
    pub selected_cut: i32,
    pub idle_cut: i32,
    pub icon_cut: i32,
}

const IDLE_CUT_BY_KIND: [i32; 4] = [0x113, 0x113, 1, 0x162];

pub fn parse_orb_effect_row(row: &mut OrbEffectRow, stm: &mut AssetStream<'_>, id: i32) {
    row.targets[0] = -1;
    row.targets[1] = -1;
    row.targets[2] = -1;
    row.id = id;
    row.kind = read_csv_cell(stm, 0) as i32;

    let slot = if (row.kind as u32) < 2 {
        1
    } else if row.kind == 2 {
        2
    } else {
        0
    };

    row.targets[slot] = read_csv_cell(stm, 1) as i32;
    row.serial = read_csv_cell(stm, 2) as i32;

    let value = read_csv_cell(stm, 3) as i32;

    row.icon_cut = if value == -1 { 0x32 } else { value };

    let value = read_csv_cell(stm, 4) as i32;

    if value != -1 {
        row.selected_cut = value;
    } else if (row.kind as u32) < 2 {
        row.selected_cut = 0x112;
    } else if row.kind == 2 {
        row.selected_cut = 0;
        row.idle_cut = 1;

        return;
    } else if row.kind == 3 {
        row.selected_cut = 0x160;
        row.idle_cut = 0x162;

        return;
    }

    if row.kind as u32 <= 3 {
        if let Some(cut) = IDLE_CUT_BY_KIND.get(row.kind as u32 as usize) {
            row.idle_cut = *cut;
        }
    }
}
