use std::rc::Rc;

use crate::Fault;

use super::{AppContext, RoguelikeProgress, get_unit_rarity};

pub fn roguelike_powerup_totals(
    ctx: &AppContext,
    progress: &RoguelikeProgress,
    unit_id: i32,
) -> Result<(i32, i32), Fault> {
    let master = Rc::clone(&ctx.roguelike.master);
    let rarity = get_unit_rarity(ctx, unit_id)?;
    let mut level = 0i32;
    let mut plus = 0i32;

    for list in [&progress.carried_bonuses, &progress.bonuses] {
        for record in list.iter() {
            if record.kind != 1 {
                continue;
            }

            let Some(buff) = master.buffs.get(&record.id) else {
                continue;
            };

            if buff.target_chara != -1 && buff.target_chara != unit_id {
                continue;
            }

            if buff.target_rarity != -1 && buff.target_rarity != rarity {
                continue;
            }

            for effect in &buff.effects {
                let Some(effect) = effect else {
                    continue;
                };

                if effect.kind() != 0 {
                    continue;
                }

                level = level.wrapping_add(record.level.wrapping_mul(effect.first()));
                plus = plus.wrapping_add(effect.second().wrapping_mul(record.level));
            }
        }
    }

    Ok((level, plus))
}
