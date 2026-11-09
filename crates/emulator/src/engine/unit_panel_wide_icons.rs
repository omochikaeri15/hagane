use std::{collections::BTreeMap, rc::Rc};

use crate::Fault;

use super::{
    AppContext, EFFECT_UP_BASE_ICONS, RoguelikeEffect, TALENT_TRAIT_ICONS, is_roguelike_map,
    roguelike_effect_table, roguelike_icon_ability, roguelike_progress, roguelike_talent_values,
    unit_panel_icon_order, unit_panel_icon_table,
};

pub fn unit_panel_wide_icons(ctx: &mut AppContext, unit_id: i32) -> Result<Vec<i32>, Fault> {
    let mut icons: Vec<i32> = Vec::new();
    let table = unit_panel_icon_table(ctx)?;
    let mut by_ability: BTreeMap<i32, i32> = BTreeMap::new();

    for (index, row) in &table {
        let ability = *row.get(2).ok_or(Fault::index_out_of_range(2, row.len() as i64))?;

        by_ability.insert(ability, *index);
    }

    if !is_roguelike_map(ctx, -1, -1)? {
        for (icon, _) in unit_panel_icon_order(ctx)? {
            let limit = ctx.picture_book_abilities.len() as i64;
            let group = *ctx
                .picture_book_abilities
                .get(icon as i64 as usize)
                .and_then(|row| row.get(1))
                .ok_or(Fault::index_out_of_range(icon as i64, limit))?;

            if group == 0 && ctx.ability_icons.get(&icon).is_some_and(|shown| *shown) {
                icons.push(icon);
            }
        }

        return Ok(icons);
    }

    let progress = roguelike_progress(ctx, -1, -1)?.ok_or(Fault::null_pointer())?;
    let master = Rc::clone(&ctx.roguelike.master);
    let mut granted: BTreeMap<i32, bool> = BTreeMap::new();
    let lists = [progress.borrow().carried_bonuses.clone(), progress.borrow().bonuses.clone()];

    for list in &lists {
        for bonus in list {
            if bonus.kind != 1 {
                continue;
            }

            let buff = master.buffs.get(&bonus.id).ok_or(Fault::null_pointer())?;

            for effect in &buff.effects {
                let Some(RoguelikeEffect::Instinct(skill)) = effect else {
                    continue;
                };
                let ability = skill.ability_id;
                let icon = by_ability.get(&ability).copied();

                if roguelike_talent_values(ctx, unit_id, ability, -1, -1)?.is_none() {
                    continue;
                }

                if TALENT_TRAIT_ICONS
                    .iter()
                    .find(|(key, _)| *key == ability)
                    .is_some_and(|(_, trait_icon)| *trait_icon != -1)
                {
                    continue;
                }

                granted.insert(icon.ok_or(Fault::key_not_found(ability as i64))?, true);
            }
        }
    }

    let mut rows = roguelike_effect_table(ctx)?;

    for (icon, _) in unit_panel_icon_order(ctx)? {
        if granted.get(&icon).is_some_and(|shown| *shown) {
            continue;
        }

        let ability = roguelike_icon_ability(icon);

        if ability != -1 && granted.get(&ability).is_some_and(|shown| *shown) {
            continue;
        }

        let row = rows.entry(icon).or_default();
        let group = *row.get(1).ok_or(Fault::index_out_of_range(1, row.len() as i64))?;

        if group == 0 && ctx.ability_icons.get(&icon).is_some_and(|shown| *shown) {
            icons.push(icon);
        }
    }

    for (icon, _) in unit_panel_icon_order(ctx)? {
        let row = rows.entry(icon).or_default();
        let group = *row.get(1).ok_or(Fault::index_out_of_range(1, row.len() as i64))?;

        if group != 0 || !granted.get(&icon).is_some_and(|shown| *shown) {
            continue;
        }

        let ability = *row.get(2).ok_or(Fault::index_out_of_range(2, row.len() as i64))?;

        if let Some((_, base)) = EFFECT_UP_BASE_ICONS.iter().find(|(key, _)| *key == ability)
            && *base != -1
            && !icons.contains(base)
        {
            continue;
        }

        icons.push(icon);
    }

    Ok(icons)
}
