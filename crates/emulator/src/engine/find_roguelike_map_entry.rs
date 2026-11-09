use crate::Fault;

use super::{AppContext, get_crown_level, get_global_map_id};

pub fn find_roguelike_map_entry(
    ctx: &mut AppContext,
    map_id: i32,
    crown: i32,
) -> Result<Option<(i32, usize)>, Fault> {
    let mut map_id = map_id;
    let mut crown = crown;

    if map_id == -1 {
        map_id = get_global_map_id(ctx, 0)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    for (event_id, event) in &ctx.roguelike.settings {
        for (index, entry) in event.entries.iter().enumerate() {
            if !entry.map_ids.contains(&map_id) {
                continue;
            }

            if !entry.levels.contains(&crown) {
                continue;
            }

            return Ok(Some((*event_id, index)));
        }
    }

    Ok(None)
}
