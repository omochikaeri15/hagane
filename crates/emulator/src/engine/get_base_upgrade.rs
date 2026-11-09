use crate::Fault;

use super::{AppContext, get_tech_level, has_fixed_lineup, is_roguelike_map};

pub fn get_base_upgrade(ctx: &mut AppContext, upgrade: i32) -> Result<i32, Fault> {
    let mut key = upgrade;
    let fixed = has_fixed_lineup(ctx, -1, -1, -1)?;
    let roguelike = is_roguelike_map(ctx, -1, -1)?;

    if fixed || roguelike {
        key = key.wrapping_sub((key > 0) as i32);

        let levels = if fixed {
            &mut ctx.fixed_lineup_store.ability_levels
        } else {
            &mut ctx.roguelike.lineup.ability_levels
        };

        if levels.contains_key(&key) {
            return Ok(*levels.entry(key).or_insert(0));
        }
    }

    get_tech_level(
        ctx,
        (((key as i64) << 3) + AppContext::TECH_LEVELS as i64) as usize,
    )
}
