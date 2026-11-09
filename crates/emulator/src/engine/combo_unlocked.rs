use crate::{Fault, ops};

use super::{
    AppContext, NyancomboRecord, UNIT_BUY, UnitBuy, find_roguelike_chara, get_tech_level,
    is_roguelike_map, roguelike_progress,
};

pub fn combo_unlocked(ctx: &mut AppContext, record: &NyancomboRecord) -> Result<bool, Fault> {
    if record.availability == -1 || record.state == 2 {
        return Ok(false);
    }

    let progress = if is_roguelike_map(ctx, -1, -1)? {
        roguelike_progress(ctx, -1, -1)?
    } else {
        None
    };

    for member in 0..5 {
        let unit = record.unit[member];

        if unit == -1 {
            return Ok(true);
        }

        let row = (unit as i64) << 8;
        let mut level = get_tech_level(
            ctx,
            AppContext::UNIT_LEVELS.wrapping_add((unit as i64 as usize).wrapping_mul(8)),
        )?;
        let mut pair = [0u8; 8];

        pair[..4].copy_from_slice(&ctx.block_at::<4>(
            AppContext::UNITS_OWNED.wrapping_add((unit as i64 as usize).wrapping_mul(4)),
        )?);
        pair[4..].copy_from_slice(&ctx.block_at::<4>(ctx.units_owned_key())?);

        let mut owned =
            ops::xor_row_decode(&pair, 1, 0).ok_or(Fault::index_out_of_range(0, 1))? as i32 > 0;
        let mut reward_units = ctx.i32_at(
            AppContext::REWARD_UNITS_OWNED.wrapping_add((unit as i64 as usize).wrapping_mul(4)),
        )?;
        let mut fourth_form = ctx.i32_at(
            AppContext::REWARD_FORMS_OWNED.wrapping_add((unit as i64 as usize).wrapping_mul(4)),
        )? >= 2;
        let true_form;
        let form;

        if let Some(progress) = &progress {
            let progress = progress.borrow();

            if let Some(chara) = find_roguelike_chara(&progress, unit) {
                let chara = &progress.roster[chara];

                level = chara.plus_level.wrapping_add(chara.level);
                reward_units = chara.top_form;
                fourth_form = reward_units >= 3;
                owned = true;
                true_form = reward_units >= 2;
                form = record.form[member];

                if form == 0 && owned {
                    continue;
                }
            } else {
                form = record.form[member];
                fourth_form = false;
                true_form = false;
            }
        } else {
            true_form = reward_units >= 2;
            form = record.form[member];

            if form == 0 && owned {
                continue;
            }
        }

        if form == 1 && level > 8 {
            continue;
        }

        if form == 2 {
            pair[..4].copy_from_slice(&ctx.block_at::<4>(
                (row + (UNIT_BUY + UnitBuy::TRUE_FORM_LEVEL) as i64) as usize,
            )?);
            pair[4..].copy_from_slice(
                &ctx.block_at::<4>((row + (UNIT_BUY + UnitBuy::KEY) as i64) as usize)?,
            );

            if ops::xor_row_decode(&pair, 1, 0).ok_or(Fault::index_out_of_range(0, 1))? as i32 != -1 {
                pair[..4].copy_from_slice(&ctx.block_at::<4>(
                    (row + (UNIT_BUY + UnitBuy::TRUE_FORM_LEVEL) as i64) as usize,
                )?);
                pair[4..].copy_from_slice(
                    &ctx.block_at::<4>((row + (UNIT_BUY + UnitBuy::KEY) as i64) as usize)?,
                );

                if level.wrapping_add(1)
                    >= ops::xor_row_decode(&pair, 1, 0).ok_or(Fault::index_out_of_range(0, 1))? as i32
                {
                    continue;
                }
            }

            pair[..4].copy_from_slice(&ctx.block_at::<4>(
                (row + (UNIT_BUY + UnitBuy::TRUE_FORM_LEVEL) as i64) as usize,
            )?);
            pair[4..].copy_from_slice(
                &ctx.block_at::<4>((row + (UNIT_BUY + UnitBuy::KEY) as i64) as usize)?,
            );

            if ops::xor_row_decode(&pair, 1, 0).ok_or(Fault::index_out_of_range(0, 1))? as i32 == -1
                && true_form
            {
                continue;
            }
        }

        if form == 3 && fourth_form {
            continue;
        }

        return Ok(false);
    }

    Ok(true)
}
