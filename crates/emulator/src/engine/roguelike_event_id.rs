use crate::Fault;

use super::{AppContext, get_crown_level, get_global_map_id};

pub fn roguelike_event_id(ctx: &mut AppContext, map_id: i32, crown: i32) -> Result<i32, Fault> {
    let mut map_id = map_id;
    let mut crown = crown;

    if map_id == -1 {
        map_id = get_global_map_id(ctx, 0)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    for (event_id, event) in &ctx.roguelike.settings {
        for entry in &event.entries {
            if !entry.map_ids.contains(&map_id) {
                continue;
            }

            if !entry.levels.contains(&crown) {
                continue;
            }

            return Ok(*event_id);
        }
    }

    Ok(-1)
}
