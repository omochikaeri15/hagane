use std::rc::Rc;

use crate::Fault;

use super::{AppContext, get_unit_rarity, roguelike_progress};

pub fn roguelike_combo_values(
    ctx: &mut AppContext,
    kind: i32,
    unit_id: i32,
    map_id: i32,
    crown: i32,
) -> Result<Option<(i32, Vec<i32>)>, Fault> {
    let Some(progress) = roguelike_progress(ctx, map_id, crown)? else {
        return Ok(None);
    };

    let progress = progress.borrow();
    let mut total = 0i32;
    let mut each: Vec<i32> = Vec::new();
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

                if effect.kind() != 2 {
                    continue;
                }

                if effect.first() != kind {
                    continue;
                }

                let band = effect.second();
                let value = *ctx
                    .combo_store
                    .params
                    .get(kind as i64 as usize)
                    .and_then(|powers| powers.get(band as i64 as usize))
                    .ok_or(Fault::index_out_of_range(band as i64, 0))?;

                total = total.wrapping_add(value.wrapping_mul(record.level));
                found = true;

                let mut copy = 0i32;

                while copy < record.level {
                    let band = effect.second();
                    let value = *ctx
                        .combo_store
                        .params
                        .get(kind as i64 as usize)
                        .and_then(|powers| powers.get(band as i64 as usize))
                        .ok_or(Fault::index_out_of_range(band as i64, 0))?;

                    each.push(value);
                    copy += 1;
                }
            }
        }
    }

    if !found {
        return Ok(None);
    }

    Ok(Some((total, each)))
}
