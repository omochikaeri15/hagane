use super::{AssetStream, get_column_count, read_csv_cell, read_csv_row};

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct GamatotoBonus {
    pub minutes: i32,
    pub max_charges: i32,
    pub steps: Vec<[i32; 2]>,
}

pub fn parse_gamatoto_bonus(out: &mut GamatotoBonus, stm: &mut AssetStream<'_>) {
    out.steps.clear();
    read_csv_row(stm);
    out.minutes = read_csv_cell(stm, 0) as i32;
    out.max_charges = read_csv_cell(stm, 1) as i32;
    read_csv_row(stm);

    let mut column = 0i32;

    while column < get_column_count(stm) as i32 {
        let hours = read_csv_cell(stm, column) as i32;
        let chance_permille = read_csv_cell(stm, column | 1) as i32;

        out.steps.push([hours, chance_permille]);
        column = column.wrapping_add(2);
    }
}
