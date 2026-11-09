use std::{collections::BTreeMap, rc::Rc};

use crate::{
    Fault,
    engine::{
        AppContext, RoguelikeBonus, RoguelikeChara, RoguelikeRun, roguelike_add_bonus, roguelike_apply_deck, roguelike_begin_run, roguelike_bonus_is_permanent,
        roguelike_cannon_levels, roguelike_deck_preset, roguelike_event_id, roguelike_progress, roguelike_refresh_roster,
    },
};

use super::{DECK_SLOTS, RosterPick, Setup};

const CURRENT: i32 = -1;
const NO_EVENT: i32 = -1;
const BUFF_KIND: i32 = 1;
const ONE_LEVEL: i32 = 1;
const NO_FORM: i32 = -1;
const NOT_OWNED: i32 = 0;

pub fn seed_roguelike_run(ctx: &mut AppContext, setup: &Setup) -> Result<(), Fault> {
    let event = roguelike_event_id(ctx, CURRENT, CURRENT)?;

    if event == NO_EVENT {
        return Ok(());
    }

    for member in setup.lineup.iter().take(DECK_SLOTS) {
        ctx.set_i32_at(AppContext::UNITS_OWNED + (member.unit as usize) * 4, NOT_OWNED)?;
    }

    let master = Rc::clone(&ctx.roguelike.master);
    let mut kept: Vec<RoguelikeBonus> = Vec::new();

    for (buff, level) in &setup.roguelike.buffs {
        if !roguelike_bonus_is_permanent(&master, event, BUFF_KIND, *buff) {
            continue;
        }

        let level = master.buffs.get(buff).map_or(0, |record| (*level).min(record.limit));

        if level > 0 {
            kept.push(RoguelikeBonus { kind: BUFF_KIND, id: *buff, level });
        }
    }

    ctx.roguelike.runs.insert(event, RoguelikeRun { maps: BTreeMap::new(), permanent_bonuses: kept });
    roguelike_begin_run(ctx, CURRENT, CURRENT)?;

    let Some(progress) = roguelike_progress(ctx, CURRENT, CURRENT)? else {
        return Ok(());
    };

    if !setup.roguelike.roster.is_empty() {
        let mut run = progress.borrow_mut();
        let started = std::mem::take(&mut run.roster);

        for pick in setup.roguelike.roster.iter().take(DECK_SLOTS) {
            let record = match pick {
                RosterPick::Starter(unit) => started.iter().find(|member| member.unit_id == *unit).copied(),
                RosterPick::Recruit(id) => master.charas.get(id).map(|chara| RoguelikeChara {
                    unit_id: chara.chara_id,
                    form: 0,
                    top_form: NO_FORM,
                    level: 0,
                    base_level: chara.level.wrapping_sub(1),
                    plus_level: 0,
                    base_plus: chara.plus_level,
                }),
            };

            if let Some(record) = record.filter(|record| !run.roster.iter().any(|member| member.unit_id == record.unit_id)) {
                run.roster.push(record);
            }
        }

        if run.roster.is_empty() {
            run.roster = started;
        }
    }

    roguelike_refresh_roster(ctx, &progress)?;

    for (buff, level) in &setup.roguelike.buffs {
        if roguelike_bonus_is_permanent(&master, event, BUFF_KIND, *buff) {
            continue;
        }

        let Some(limit) = master.buffs.get(buff).map(|record| record.limit) else {
            continue;
        };

        for _ in 0..(*level).min(limit).max(0) {
            roguelike_add_bonus(ctx, BUFF_KIND, *buff, ONE_LEVEL, CURRENT, CURRENT)?;
        }
    }

    let unlocked = roguelike_cannon_levels(ctx, &progress.borrow()).contains_key(&setup.roguelike.cannon);

    if unlocked {
        progress.borrow_mut().cannon_part = setup.roguelike.cannon;
    }

    progress.borrow_mut().stage_index = setup.stage.stage;
    roguelike_apply_deck(ctx, CURRENT, CURRENT)?;
    ctx.set_i32_at(AppContext::SELECTED_DECK_PRESET, roguelike_deck_preset())
}
