use crate::Fault;

use super::{AppContext, get_stage_index, roguelike_progress};

pub fn roguelike_mark_stage(ctx: &mut AppContext) -> Result<(), Fault> {
    if let Some(progress) = roguelike_progress(ctx, -1, -1)? {
        let stage = get_stage_index(ctx)?;

        progress.borrow_mut().stage_index = stage;
    }

    Ok(())
}
