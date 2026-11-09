use crate::{Fault, ops};

use super::{
    AppContext, RoguelikeProgress, get_setting, get_unit_form_count, min_i32,
    roguelike_powerup_totals,
};

pub fn roguelike_unit_level(
    ctx: &AppContext,
    progress: &RoguelikeProgress,
    unit_id: i32,
    level: i32,
    plus: i32,
) -> Result<(i32, i32, i32), Fault> {
    let (level_gain, plus_gain) = roguelike_powerup_totals(ctx, progress, unit_id)?;
    let mut plus_total = plus_gain.wrapping_add(plus);
    let mut pair = [0u8; 8];

    pair[..4].copy_from_slice(&ctx.block_at::<4>(
        AppContext::UNITS_OWNED.wrapping_add((unit_id as i64 as usize).wrapping_mul(4)),
    )?);
    pair[4..].copy_from_slice(&ctx.block_at::<4>(ctx.units_owned_key())?);

    if ops::xor_row_decode(&pair, 1, 0).ok_or(Fault::index_out_of_range(0, 1))? as i32 > 0 {
        plus_total = get_setting(&ctx.settings, b"roguelike_charabonus_plusvalue", 5)?
            .wrapping_add(plus_total);
    }

    let level_total = level_gain.wrapping_add(level);
    let sum = plus_total.wrapping_add(level_total);
    let form = if sum > 0x3a {
        3
    } else if sum > 0x1c {
        2
    } else {
        (sum >= 9) as i32
    };
    let form = min_i32(form, get_unit_form_count(ctx, unit_id)?.wrapping_sub(1));

    Ok((level_total, plus_total, form))
}
