use std::{cell::RefCell, rc::Rc};

use crate::Fault;

use super::{AppContext, RoguelikeProgress, get_crown_level, get_global_map_id, roguelike_event_id};

pub fn roguelike_progress(
    ctx: &mut AppContext,
    map_id: i32,
    crown: i32,
) -> Result<Option<Rc<RefCell<RoguelikeProgress>>>, Fault> {
    let mut map_id = map_id;
    let mut crown = crown;

    if map_id == -1 {
        map_id = get_global_map_id(ctx, 0)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    let event_id = roguelike_event_id(ctx, map_id, crown)?;

    if event_id == -1 {
        return Ok(None);
    }

    if !ctx.roguelike.runs.contains_key(&event_id) {
        return Ok(None);
    }

    let current_map = get_global_map_id(ctx, 0)?;
    let current_crown = get_crown_level(ctx)?;

    Ok(ctx
        .roguelike
        .runs
        .get(&event_id)
        .and_then(|run| run.maps.get(&(current_map, current_crown)))
        .cloned())
}
