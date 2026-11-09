use crate::{Fault, ops};

use super::{AppContext, get_setting, get_unit_rarity, roguelike_progress};

pub fn roguelike_point_bonus(ctx: &mut AppContext, map_id: i32, crown: i32) -> Result<i32, Fault> {
    let mut percent = 0x64i32;
    let Some(progress) = roguelike_progress(ctx, map_id, crown)? else {
        return Ok(percent);
    };

    for chara in progress.borrow().roster.iter() {
        let mut pair = [0u8; 8];

        pair[..4].copy_from_slice(&ctx.block_at::<4>(
            AppContext::UNITS_OWNED.wrapping_add((chara.unit_id as i64 as usize).wrapping_mul(4)),
        )?);
        pair[4..].copy_from_slice(&ctx.block_at::<4>(ctx.units_owned_key())?);

        if ops::xor_row_decode(&pair, 1, 0).ok_or(Fault::index_out_of_range(0, 1))? as i32 <= 0 {
            continue;
        }

        let mut key = get_unit_rarity(ctx, chara.unit_id)?.to_string().into_bytes();

        key.splice(0..0, b"roguelike_charabonus_pointvalue".iter().copied());

        let value = get_setting(&ctx.settings, &key, 0x64)?;

        percent = percent.wrapping_add(value).wrapping_add(-0x64);
    }

    Ok(percent)
}
