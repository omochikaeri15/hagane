use crate::{Fault, ops};

use super::{
    AppContext, get_base_upgrade, get_castle_hp_growth, get_cat_combo_bonus, get_treasure_value,
    has_fixed_lineup, is_roguelike_map,
};

pub fn compute_base_health(ctx: &mut AppContext) -> Result<i32, Fault> {
    let first = get_base_upgrade(ctx, 6)?;
    let second = get_base_upgrade(ctx, 6)?;
    let base = if first <= 3 {
        second.wrapping_mul(1000).wrapping_add(1000)
    } else {
        let third = get_base_upgrade(ctx, 6)?;

        if second <= 7 {
            third.wrapping_mul(2000).wrapping_add(-2000)
        } else {
            third.wrapping_mul(3000).wrapping_add(-9000)
        }
    };
    let mut level = -1i32;

    if has_fixed_lineup(ctx, -1, -1, -1)? {
        if ctx.i32_at(AppContext::LINEUP_BASE_LEVEL)? != -1 {
            level = ctx.i32_at(AppContext::LINEUP_BASE_LEVEL)?.wrapping_add(1);
        }
    } else if is_roguelike_map(ctx, -1, -1)? {
        let raised = ctx.roguelike.lineup.base_level.wrapping_add(1);

        if raised != 0 {
            level = raised;
        }
    }

    let growth = get_castle_hp_growth(ctx, level)?;
    let treasure = get_treasure_value(ctx, &ctx.treasure_store, 0)?;
    let total = base.wrapping_add(treasure).wrapping_add(growth);
    let bonus = get_cat_combo_bonus(ctx, 0xa, -1)?;

    Ok(ops::div_100(bonus.wrapping_add(100).wrapping_mul(total) as i64) as i32)
}
