use std::{cell::RefCell, rc::Rc};

use crate::Fault;

use super::{
    AppContext, RoguelikeChara, RoguelikePending, RoguelikeProgress, find_roguelike_map_entry,
    get_crown_level, get_global_map_id, get_stage_count, load_lineup_preset, map_index_of_map_id,
    map_type_of_map_id, request_save_data, roguelike_event_id, roguelike_permanent_items,
    roguelike_unit_level,
};

pub fn roguelike_begin_run(ctx: &mut AppContext, map_id: i32, crown: i32) -> Result<(), Fault> {
    let mut map_id = map_id;
    let mut crown = crown;

    if map_id == -1 {
        map_id = get_global_map_id(ctx, 0)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    let Some((entry_event, entry_index)) = find_roguelike_map_entry(ctx, map_id, crown)? else {
        return Ok(());
    };
    let event_id = roguelike_event_id(ctx, map_id, crown)?;

    if event_id == -1 {
        return Ok(());
    }

    let run = ctx.roguelike.runs.entry(event_id).or_default();
    let existed = run.maps.contains_key(&(map_id, crown));
    let progress = Rc::clone(run.maps.entry((map_id, crown)).or_insert_with(|| {
        Rc::new(RefCell::new(RoguelikeProgress { pending_stage: -1, ..Default::default() }))
    }));
    let map_type = map_type_of_map_id(map_id);
    let map_index = map_index_of_map_id(map_id);
    let stage = progress.borrow().stage_index;
    let mut build = !existed;

    if stage == -1 || stage >= get_stage_count(ctx, map_type, map_index)? {
        *progress.borrow_mut() = RoguelikeProgress { pending_stage: -1, ..Default::default() };
        build = true;
    }

    if build {
        let name = ctx
            .roguelike
            .settings
            .get(&entry_event)
            .and_then(|event| event.entries.get(entry_index))
            .map(|entry| entry.preset_file_name.clone())
            .ok_or(Fault::null_pointer())?;

        load_lineup_preset(ctx, true, &name)?;

        for unit in &ctx.roguelike.lineup.units {
            progress.borrow_mut().roster.push(RoguelikeChara {
                unit_id: unit.unit_id,
                form: 0,
                top_form: -1,
                level: 0,
                base_level: unit.level,
                plus_level: 0,
                base_plus: unit.plus_level,
            });
        }

        let master = Rc::clone(&ctx.roguelike.master);
        let mut lists: Vec<Vec<(i32, i32)>> = Vec::new();

        roguelike_permanent_items(&master, event_id, map_id, crown, &mut lists);

        let kept = ctx.roguelike.runs.entry(event_id).or_default().permanent_bonuses.clone();
        let mut notice = 0i32;

        for bonus in &kept {
            if !lists.iter().any(|items| items.contains(&(bonus.kind, bonus.id))) {
                continue;
            }

            progress.borrow_mut().carried_bonuses.push(*bonus);

            if bonus.kind != 0 {
                continue;
            }

            let chara = master.charas.get(&bonus.id).ok_or(Fault::null_pointer())?;

            if progress.borrow().roster.iter().any(|member| member.unit_id == chara.chara_id) {
                continue;
            }

            progress.borrow_mut().roster.push(RoguelikeChara {
                unit_id: chara.chara_id,
                form: 0,
                top_form: -1,
                level: 0,
                base_level: chara.level.wrapping_sub(1),
                plus_level: 0,
                base_plus: chara.plus_level,
            });
            notice = 2;
        }

        progress.borrow_mut().pending.push_back(RoguelikePending { kind: notice, unknown: [0; 9] });
    }

    if ctx.roguelike.start_notice_queued == 0 {
        progress.borrow_mut().pending.push_back(RoguelikePending { kind: 3, unknown: [0; 9] });
        ctx.roguelike.start_notice_queued = 1;
    }

    let mut member = 0usize;

    while member < progress.borrow().roster.len() {
        let chara = progress.borrow().roster[member];
        let (level, plus, form) =
            roguelike_unit_level(ctx, &progress.borrow(), chara.unit_id, chara.base_level, chara.base_plus)?;
        let mut run_progress = progress.borrow_mut();
        let chara = &mut run_progress.roster[member];

        chara.level = level;
        chara.plus_level = plus;

        if chara.top_form < form {
            chara.top_form = form;
            chara.form = form;
        }

        member += 1;
    }

    if progress.borrow().pending_stage != -1 {
        return Err(Fault::host_missing());
    }

    request_save_data(ctx)
}
