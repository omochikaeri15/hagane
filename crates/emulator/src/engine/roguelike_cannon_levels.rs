use std::{collections::BTreeMap, rc::Rc};

use super::{AppContext, RoguelikeProgress, std_map_int_int_from_list};

pub fn roguelike_cannon_levels(ctx: &AppContext, progress: &RoguelikeProgress) -> BTreeMap<i32, i32> {
    let mut levels = std_map_int_int_from_list(&[(0, 1)]);
    let master = Rc::clone(&ctx.roguelike.master);

    for list in [&progress.carried_bonuses, &progress.bonuses] {
        for record in list.iter() {
            if record.kind != 1 {
                continue;
            }

            let Some(buff) = master.buffs.get(&record.id) else {
                continue;
            };

            for effect in &buff.effects {
                let Some(effect) = effect else {
                    continue;
                };

                if effect.kind() != 3 {
                    continue;
                }

                let level = effect.second();
                let cannon = effect.first();

                *levels.entry(cannon).or_insert(0) = level;
            }
        }
    }

    levels
}
