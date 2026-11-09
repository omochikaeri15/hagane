use crate::Fault;

use super::{AppContext, combo_unlocked};

pub fn combo_lock_unmet(ctx: &mut AppContext) -> Result<(), Fault> {
    let mut index = 0usize;

    while index != ctx.combo_store.records.len() {
        let record = ctx.combo_store.records[index].clone();
        let limit = ctx.combo_store.states.len() as i64;
        let state = *ctx
            .combo_store
            .states
            .get(record.name_index as i64 as usize)
            .ok_or(Fault::index_out_of_range(record.name_index as i64, limit))?;

        if state != 2 && !combo_unlocked(ctx, &record)? {
            *ctx.combo_store
                .states
                .get_mut(record.name_index as i64 as usize)
                .ok_or(Fault::index_out_of_range(record.name_index as i64, limit))? = 1;
            ctx.combo_store.records[index].state = 1;
        }

        index += 1;
    }

    Ok(())
}
