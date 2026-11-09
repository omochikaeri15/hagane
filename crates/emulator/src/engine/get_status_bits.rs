use crate::{Fault, ops};

use super::{
    AppContext, Entity, get_button_unit_form, get_entity_button, get_setting, has_orb, has_talent,
};

pub fn get_status_bits(ctx: &mut AppContext, faction: i32, slot: i32) -> Result<i32, Fault> {
    let mut bits = 0i32;

    if ctx.i32_at(AppContext::entity_field(
        faction,
        slot,
        Entity::STRENGTHEN_THRESHOLD,
    ))? != 0
    {
        let hp = ctx.i32_at(AppContext::entity_field(faction, slot, Entity::HP))?;
        let max_hp = ctx.i32_at(AppContext::entity_field(faction, slot, Entity::MAX_HP))?;
        let scaled = max_hp.wrapping_mul(ctx.i32_at(AppContext::entity_field(
            faction,
            slot,
            Entity::STRENGTHEN_THRESHOLD,
        ))?);

        bits = (hp <= ops::div_100(scaled as i64) as i32) as i32;
    }

    if faction != 0 {
        return Ok(bits);
    }

    let occupant = ctx.i32_at(AppContext::entity_field(0, slot, Entity::OCCUPANT))?;

    let form = get_button_unit_form(ctx, 0, get_entity_button(ctx, 0, slot)?)?;
    let kill_count = ctx.i32_at(AppContext::entity_field(0, slot, Entity::KILL_COUNT))?;

    if kill_count < get_setting(&ctx.settings, b"battle_powerup_kill_enemy", 0xf)? {
        return Ok(bits);
    }

    let unit_id = occupant.wrapping_add(-2);

    if form >= 2 && has_orb(ctx, &ctx.orb_store, unit_id, 0x12)? {
        bits |= 2;
    } else if has_talent(ctx, 0, unit_id, form, 0x3f0)? {
        bits |= 2;
    }

    Ok(bits)
}
