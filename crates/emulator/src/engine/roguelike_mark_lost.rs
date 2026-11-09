use crate::Fault;

use super::{AppContext, roguelike_progress};

pub fn roguelike_mark_lost(ctx: &mut AppContext) -> Result<(), Fault> {
    if let Some(progress) = roguelike_progress(ctx, -1, -1)? {
        progress.borrow_mut().stage_index = -1;
    }

    Ok(())
}
