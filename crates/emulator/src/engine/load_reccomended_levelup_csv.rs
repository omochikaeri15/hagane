use crate::Fault;

use super::{AppContext, AssetStream, cell_is_int, open_asset_stream, read_csv_cell, read_csv_row};

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RecommendedLevelup {
    pub shown: i32,
    pub discount: i32,
    pub level_limit: i32,
    pub forms: u8,
    pub unread: i32,
    pub pool: i32,
}

pub fn load_reccomended_levelup_csv(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.recommended_levelups.clear();

    let Some(bytes) = open_asset_stream(ctx, b"Recommended_levelup.csv", 0, 0)? else {
        return Ok(());
    };
    let stm = &mut AssetStream::new(&bytes, b'\n');

    while read_csv_row(stm) {
        if !cell_is_int(stm, 0) {
            break;
        }

        let key = read_csv_cell(stm, 0) as i32;
        let value = read_csv_cell(stm, 1) as i32;

        ctx.recommended_levelups.entry(key).or_default().shown = value;

        let value = read_csv_cell(stm, 2) as i32;

        ctx.recommended_levelups.entry(key).or_default().discount = value;

        let value = read_csv_cell(stm, 3) as i32;

        ctx.recommended_levelups.entry(key).or_default().level_limit = value;

        let value = read_csv_cell(stm, 4) as i32;

        ctx.recommended_levelups.entry(key).or_default().forms = u8::from(value != 0);

        let value = read_csv_cell(stm, 5) as i32;

        ctx.recommended_levelups.entry(key).or_default().unread = value;

        let value = read_csv_cell(stm, 6) as i32;

        ctx.recommended_levelups.entry(key).or_default().pool = value;
    }

    Ok(())
}
