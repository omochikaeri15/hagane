use crate::Fault;

use super::{AppContext, find_roguelike_chara, is_roguelike_map, roguelike_progress};

pub fn roguelike_unit_form(ctx: &mut AppContext, unit_id: i32) -> Result<i32, Fault> {
    if is_roguelike_map(ctx, -1, -1)?
        && let Some(progress) = roguelike_progress(ctx, -1, -1)?
    {
        let progress = progress.borrow();

        if let Some(chara) = find_roguelike_chara(&progress, unit_id) {
            return Ok(progress.roster[chara].form);
        }
    }

    ctx.i32_at(AppContext::UNIT_FORMS.wrapping_add((unit_id as i64 as usize).wrapping_mul(4)))
}
