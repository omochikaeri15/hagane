use crate::{Fault, ops};

use super::{
    AppContext, AssetStream, cell_is_int, get_setting_double, open_asset_stream, read_csv_cell,
    read_csv_cell_double, read_csv_row,
};

pub fn load_vibration_csv(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.battle_event_latch.gates.clear();

    if let Some(bytes) = open_asset_stream(ctx, b"Vibration.csv", 0, 0)? {
        let stm = &mut AssetStream::new(&bytes, b'\n');

        while read_csv_row(stm) {
            if !cell_is_int(stm, 0) {
                break;
            }

            let id = read_csv_cell(stm, 0) as i32;
            let kind = read_csv_cell(stm, 1) as i32;

            if kind != -1 && kind != 1 {
                continue;
            }

            let value = read_csv_cell_double(stm, 2);
            let group = ops::div_2(id).wrapping_add(1);
            let slot = id.wrapping_sub(ops::div_2(id).wrapping_mul(2));

            ctx.battle_event_latch
                .gates
                .entry(group)
                .or_default()
                .entry(slot)
                .or_default()[0] = value;

            let value = read_csv_cell_double(stm, 3) / 1000.0;

            ctx.battle_event_latch
                .gates
                .entry(group)
                .or_default()
                .entry(slot)
                .or_default()[1] = value;

            let value = read_csv_cell_double(stm, 4);

            ctx.battle_event_latch
                .gates
                .entry(group)
                .or_default()
                .entry(slot)
                .or_default()[2] = value;
        }
    }

    ctx.battle_event_latch.power =
        get_setting_double(&ctx.settings, b"option_vibration_power_android", 0.0)?;
    ctx.battle_event_latch.sample_seconds =
        get_setting_double(&ctx.settings, b"option_vibration_time_android", 0.0)? / 1000.0;
    ctx.battle_event_latch.tail = -1.0;

    Ok(())
}
