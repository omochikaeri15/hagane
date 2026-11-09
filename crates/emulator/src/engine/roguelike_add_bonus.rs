use std::rc::Rc;

use crate::Fault;

use super::{
    AppContext, RoguelikeBonus, get_crown_level, get_global_map_id, roguelike_bonus_is_permanent,
    roguelike_event_id, roguelike_unit_level,
};

pub fn roguelike_add_bonus(
    ctx: &mut AppContext,
    kind: i32,
    id: i32,
    level: i32,
    map_id: i32,
    crown: i32,
) -> Result<(), Fault> {
    let mut resolved_map = map_id;

    if map_id == -1 {
        resolved_map = get_global_map_id(ctx, 0)?;
    }

    let mut resolved_crown = crown;

    if crown == -1 {
        resolved_crown = get_crown_level(ctx)?;
    }

    let event_id = roguelike_event_id(ctx, resolved_map, resolved_crown)?;

    if event_id == -1 || !ctx.roguelike.runs.contains_key(&event_id) {
        return Ok(());
    }

    let current_map = get_global_map_id(ctx, 0)?;
    let current_crown = get_crown_level(ctx)?;
    let Some(progress) = ctx
        .roguelike
        .runs
        .get(&event_id)
        .and_then(|run| run.maps.get(&(current_map, current_crown)))
        .cloned()
    else {
        return Ok(());
    };

    let owner = roguelike_event_id(ctx, map_id, crown)?;

    if owner == -1 {
        return Ok(());
    }

    let master = Rc::clone(&ctx.roguelike.master);
    let permanent = roguelike_bonus_is_permanent(&master, owner, kind, id);

    {
        let mut run_progress = progress.borrow_mut();
        let list = if permanent {
            &mut ctx
                .roguelike
                .runs
                .get_mut(&event_id)
                .ok_or(Fault::null_pointer())?
                .permanent_bonuses
        } else {
            &mut run_progress.bonuses
        };
        let mut index = 0usize;

        while index != list.len() {
            if list[index].kind == kind && list[index].id == id {
                break;
            }

            index += 1;
        }

        if index != list.len() {
            list[index].level = list[index].level.wrapping_add(level);
        } else {
            list.push(RoguelikeBonus { kind, id, level });
        }
    }

    let mut index = 0usize;

    while index != progress.borrow().roster.len() {
        let chara = progress.borrow().roster[index];
        let (level, plus, form) = roguelike_unit_level(
            ctx,
            &progress.borrow(),
            chara.unit_id,
            chara.base_level,
            chara.base_plus,
        )?;
        let mut run_progress = progress.borrow_mut();
        let chara = &mut run_progress.roster[index];

        chara.level = level;
        chara.plus_level = plus;

        if chara.top_form < form {
            chara.top_form = form;
            chara.form = form;
        }

        index += 1;
    }

    Ok(())
}
