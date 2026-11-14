use crate::Fault;

use super::{
    AppContext, AssetStream, cell_is_int, get_column_count, open_asset_stream, read_csv_cell,
    read_csv_row,
};

pub fn load_change_conditions_csv(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.labyrinth_lottery.clear();

    let Some(bytes) = open_asset_stream(ctx, b"change_conditions.csv", 0, 0)? else {
        return Ok(());
    };
    let stm = &mut AssetStream::new(&bytes, b'\n');

    while read_csv_row(stm) {
        if !cell_is_int(stm, 0) {
            break;
        }

        let key = read_csv_cell(stm, 0) as i32;
        let value = read_csv_cell(stm, 1) as i32;

        ctx.labyrinth_lottery.entry(key).or_default().min = value;

        let value = read_csv_cell(stm, 2) as i32;

        ctx.labyrinth_lottery.entry(key).or_default().max = value;
        ctx.labyrinth_lottery.entry(key).or_default().results.clear();
        ctx.labyrinth_lottery.entry(key).or_default().weights.clear();

        let mut column = 3i32;

        while column.wrapping_add(1) < get_column_count(stm) as i32 {
            if !cell_is_int(stm, column) {
                break;
            }

            if !cell_is_int(stm, column.wrapping_add(1)) {
                break;
            }

            let value = read_csv_cell(stm, column) as i32;

            ctx.labyrinth_lottery.entry(key).or_default().results.push(value);

            let value = read_csv_cell(stm, column.wrapping_add(1)) as i32;

            ctx.labyrinth_lottery.entry(key).or_default().weights.push(value);

            column = column.wrapping_add(2);
        }
    }

    Ok(())
}
