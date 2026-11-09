use std::rc::Rc;

use crate::{Fault, ops};

use super::{AppContext, RoguelikeEffect, get_unit_rarity, roguelike_progress};

pub fn roguelike_talent_values(
    ctx: &mut AppContext,
    unit_id: i32,
    ability: i32,
    map_id: i32,
    crown: i32,
) -> Result<Option<Vec<i32>>, Fault> {
    let Some(progress) = roguelike_progress(ctx, map_id, crown)? else {
        return Ok(None);
    };

    let progress = progress.borrow();
    let mut values: Vec<i32> = Vec::new();
    let mut found = false;

    for list in [&progress.carried_bonuses, &progress.bonuses] {
        for record in list.iter() {
            if record.kind != 1 {
                continue;
            }

            let master = Rc::clone(&ctx.roguelike.master);
            let Some(buff) = master.buffs.get(&record.id) else {
                continue;
            };

            if buff.target_chara != -1 && buff.target_chara != unit_id {
                continue;
            }

            if buff.target_rarity != -1 && buff.target_rarity != get_unit_rarity(ctx, unit_id)? {
                continue;
            }

            for effect in &buff.effects {
                let Some(effect) = effect else {
                    continue;
                };

                if effect.kind() != 1 {
                    continue;
                }

                if effect.first() != ability {
                    continue;
                }

                let RoguelikeEffect::Instinct(skill) = effect else {
                    return Err(Fault::null_pointer());
                };
                let params = &skill.params;

                found = true;

                if params.is_empty() {
                    continue;
                }

                if values.is_empty() {
                    values.resize(((params.len() - 1) >> 1) + 1, 0);
                }

                if effect.second() < 2 {
                    if params.len() < 2 {
                        continue;
                    }

                    let last = (params.len() - 2) >> 1;
                    let mut pair = 0usize;

                    loop {
                        let limit = values.len() as i64;
                        let cell = values
                            .get_mut(pair)
                            .ok_or(Fault::index_out_of_range(pair as i64, limit))?;

                        *cell = cell.wrapping_add(params[pair * 2]);

                        if pair == last {
                            break;
                        }

                        pair += 1;
                    }
                } else {
                    if params.len() * 4 < 5 {
                        continue;
                    }

                    let mut cell = 1usize;

                    loop {
                        let min = params[cell - 1];
                        let max = params[cell];
                        let level = record.level;
                        let steps = effect.second().wrapping_sub(1);
                        let scaled = max.wrapping_sub(min).wrapping_mul(level.wrapping_sub(1));
                        let quotient = ops::idiv(scaled, steps).ok_or(Fault::divide(steps as i64))?;
                        let limit = values.len() as i64;
                        let slot = values
                            .get_mut((cell - 1) / 2)
                            .ok_or(Fault::index_out_of_range(((cell - 1) / 2) as i64, limit))?;

                        *slot = slot.wrapping_add(quotient.wrapping_add(min));
                        cell += 2;

                        if params.len() <= cell {
                            break;
                        }
                    }
                }
            }
        }
    }

    if !found {
        return Ok(None);
    }

    Ok(Some(values))
}
