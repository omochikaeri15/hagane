use crate::Fault;

use super::{
    AppContext, AssetStream, EnigmaEntry, EnigmaLottery, EnigmaPool, EnigmaTier, cell_is_int,
    get_column_count, open_asset_stream, read_cell_stream, read_csv_cell, read_csv_row,
    std_to_string,
};

pub fn load_hidden_data_csv(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.enigma.lotteries.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Hidden_rate.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) {
            if (get_column_count(stm) as i64) < 2 {
                break;
            }

            if read_cell_stream(stm, 0) != std_to_string(read_csv_cell(stm, 0) as i32).as_slice() {
                break;
            }

            ctx.enigma.tiers.push(EnigmaTier::default());

            let mut column = 0i32;

            while column | 1 < get_column_count(stm) as i32 {
                if read_cell_stream(stm, column)
                    != std_to_string(read_csv_cell(stm, column) as i32).as_slice()
                {
                    break;
                }

                let first = read_csv_cell(stm, column) as i32;

                if let Some(row) = ctx.enigma.tiers.last_mut() {
                    row.thresholds.push(first);
                }

                let second = read_csv_cell(stm, column | 1) as i32;

                if let Some(row) = ctx.enigma.tiers.last_mut() {
                    row.chances.push(second);
                }

                column = column.wrapping_add(2);
            }
        }
    }

    ctx.enigma.pools.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Hidden_drop.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) {
            if (get_column_count(stm) as i64) < 2 {
                break;
            }

            if read_cell_stream(stm, 0) != std_to_string(read_csv_cell(stm, 0) as i32).as_slice() {
                break;
            }

            ctx.enigma.pools.push(EnigmaPool::default());

            let mut column = 0i32;

            while column | 1 < get_column_count(stm) as i32 {
                if read_cell_stream(stm, column)
                    != std_to_string(read_csv_cell(stm, column) as i32).as_slice()
                {
                    break;
                }

                let first = read_csv_cell(stm, column) as i32;

                if let Some(row) = ctx.enigma.pools.last_mut() {
                    row.lotteries.push(first);
                }

                let second = read_csv_cell(stm, column | 1) as i32;

                if let Some(row) = ctx.enigma.pools.last_mut() {
                    row.weights.push(second);
                }

                column = column.wrapping_add(2);
            }
        }
    }

    ctx.enigma.table.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Hidden_group.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) {
            if (get_column_count(stm) as i64) < 2 {
                break;
            }

            if !cell_is_int(stm, 0) {
                break;
            }

            let map = read_csv_cell(stm, 0) as i32;
            let stage = read_csv_cell(stm, 1) as i32;

            ctx.enigma
                .table
                .entry(map)
                .or_default()
                .insert(stage, EnigmaEntry::default());

            let mut column = 2i32;

            while column | 1 < get_column_count(stm) as i32 {
                if !cell_is_int(stm, column) {
                    break;
                }

                let first = read_csv_cell(stm, column) as i32;

                if let Some(row) = ctx
                    .enigma
                    .table
                    .entry(map)
                    .or_default()
                    .get_mut(&stage)
                {
                    row.lotteries.push(first);
                }

                let second = read_csv_cell(stm, column | 1) as i32;

                if let Some(row) = ctx
                    .enigma
                    .table
                    .entry(map)
                    .or_default()
                    .get_mut(&stage)
                {
                    row.chances.push(second);
                }

                column = column.wrapping_add(2);
            }
        }
    }

    ctx.enigma.lotteries.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Hidden_lottery.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) {
            if (get_column_count(stm) as i64) < 4 {
                break;
            }

            if read_cell_stream(stm, 0) != std_to_string(read_csv_cell(stm, 0) as i32).as_slice() {
                break;
            }

            ctx.enigma.lotteries.push(EnigmaLottery::default());

            let first = read_csv_cell(stm, 0) as i32;
            let second = read_csv_cell(stm, 1) as i32;
            let third = read_csv_cell(stm, 2) as i32;

            if let Some(row) = ctx.enigma.lotteries.last_mut() {
                row.rarity = first;
                row.appraisal_minutes = second;
                row.condition_stage = third;
            }

            let mut column = 3i32;

            while column.wrapping_add(1) < get_column_count(stm) as i32 {
                if read_cell_stream(stm, column)
                    != std_to_string(read_csv_cell(stm, column) as i32).as_slice()
                {
                    break;
                }

                let first = read_csv_cell(stm, column) as i32;

                if let Some(row) = ctx.enigma.lotteries.last_mut() {
                    row.map_ids.push(first);
                }

                let second = read_csv_cell(stm, column.wrapping_add(1)) as i32;

                if let Some(row) = ctx.enigma.lotteries.last_mut() {
                    row.weights.push(second);
                }

                column = column.wrapping_add(2);
            }
        }
    }

    ctx.enigma.medals.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Discoverlv_growth.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) && get_column_count(stm) as i64 > 0 {
            if read_cell_stream(stm, 0) != std_to_string(read_csv_cell(stm, 0) as i32).as_slice() {
                break;
            }

            let value = read_csv_cell(stm, 0) as i32;

            ctx.enigma.medals.push(value);
        }
    }

    ctx.enigma.times.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Hidden_time.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) && get_column_count(stm) as i64 > 0 {
            if read_cell_stream(stm, 0) != std_to_string(read_csv_cell(stm, 0) as i32).as_slice() {
                break;
            }

            let value = read_csv_cell(stm, 0) as i32;

            ctx.enigma.times.push(value);
        }
    }

    ctx.enigma.rarities.clear();

    let Some(bytes) = open_asset_stream(ctx, b"Hidden_rarity.csv", 0, 0)? else {
        return Ok(());
    };
    let stm = &mut AssetStream::new(&bytes, b'\n');

    while read_csv_row(stm) {
        if !cell_is_int(stm, 0) {
            break;
        }

        let key = read_csv_cell(stm, 0) as i32;
        let text = read_cell_stream(stm, 1).to_vec();

        ctx.enigma.rarities.entry(key).or_default().name = text;

        let cut = read_csv_cell(stm, 2) as i32;

        ctx.enigma.rarities.entry(key).or_default().cut = cut;
    }

    Ok(())
}
