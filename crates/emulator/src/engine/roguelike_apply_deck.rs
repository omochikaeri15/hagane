use crate::Fault;

use super::{
    AppContext, get_crown_level, get_global_map_id, roguelike_deck_preset, roguelike_event_id,
    roguelike_progress,
};

pub fn roguelike_apply_deck(ctx: &mut AppContext, map_id: i32, crown: i32) -> Result<(), Fault> {
    let mut map_id = map_id;
    let mut crown = crown;

    if map_id == -1 {
        map_id = get_global_map_id(ctx, 0)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    if roguelike_event_id(ctx, map_id, crown)? == -1 {
        return Ok(());
    }

    for slot in 0..10usize {
        let deck = (roguelike_deck_preset() as i64 as usize).wrapping_mul(AppContext::DECK_STRIDE);
        let key = u32::from_le_bytes(
            ctx.block_at::<4>(AppContext::DECK_PRESETS.wrapping_add(AppContext::DECK_KEY).wrapping_add(deck))?,
        );

        ctx.set_block_at::<4>(
            AppContext::DECK_PRESETS.wrapping_add(deck).wrapping_add(slot * 4),
            (!key).to_le_bytes(),
        )?;
    }

    let parts = (roguelike_deck_preset() as i64 as usize).wrapping_mul(3);

    ctx.set_block_at::<1>(AppContext::PRESET_CANNON_PARTS.wrapping_add(parts), [0])?;

    let parts = (roguelike_deck_preset() as i64 as usize).wrapping_mul(3);

    ctx.set_block_at::<1>(AppContext::PRESET_STYLE_PARTS.wrapping_add(parts), [0])?;

    let parts = (roguelike_deck_preset() as i64 as usize).wrapping_mul(3);

    ctx.set_block_at::<1>(AppContext::PRESET_FOUNDATION_PARTS.wrapping_add(parts), [0])?;

    let Some(progress) = roguelike_progress(ctx, map_id, crown)? else {
        return Ok(());
    };
    let progress = progress.borrow();
    let mut member = 0usize;

    while member < progress.roster.len() {
        if member >= 10 {
            return Err(Fault::index_out_of_range(member as i64, 10));
        }

        let deck = (roguelike_deck_preset() as i64 as usize).wrapping_mul(AppContext::DECK_STRIDE);
        let key = u32::from_le_bytes(
            ctx.block_at::<4>(AppContext::DECK_PRESETS.wrapping_add(AppContext::DECK_KEY).wrapping_add(deck))?,
        );
        let unit = (progress.roster[member].unit_id.wrapping_add(2) as u32) ^ key;

        ctx.set_block_at::<4>(
            AppContext::DECK_PRESETS.wrapping_add(deck).wrapping_add(member * 4),
            unit.to_le_bytes(),
        )?;
        member += 1;
    }

    let parts = (roguelike_deck_preset() as i64 as usize).wrapping_mul(3);

    ctx.set_block_at::<1>(
        AppContext::PRESET_CANNON_PARTS.wrapping_add(parts),
        [progress.cannon_part as u8],
    )?;

    let parts = (roguelike_deck_preset() as i64 as usize).wrapping_mul(3);

    ctx.set_block_at::<1>(
        AppContext::PRESET_STYLE_PARTS.wrapping_add(parts),
        [progress.style_part as u8],
    )?;

    let parts = (roguelike_deck_preset() as i64 as usize).wrapping_mul(3);

    ctx.set_block_at::<1>(
        AppContext::PRESET_FOUNDATION_PARTS.wrapping_add(parts),
        [progress.foundation_part as u8],
    )
}
