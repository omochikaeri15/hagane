use std::{collections::BTreeMap, rc::Rc};

use crate::Fault;

use super::{AppContext, RoguelikeEffect};

pub fn roguelike_effect_table(ctx: &mut AppContext) -> Result<BTreeMap<i32, Vec<i32>>, Fault> {
    let master = Rc::clone(&ctx.roguelike.master);

    if ctx.roguelike_effect_rows.is_empty() {
        let mut index = 0usize;

        while index < ctx.picture_book_abilities.len() {
            let row = ctx.picture_book_abilities[index].clone();

            ctx.roguelike_effect_rows.insert(index as i32, row);
            index += 1;
        }

        for buff in master.buffs.values() {
            for effect in &buff.effects {
                let Some(RoguelikeEffect::Instinct(skill)) = effect else {
                    continue;
                };
                let ability = skill.ability_id;

                if ctx.picture_book_abilities.len() as u64 <= ability as i64 as u64 {
                    let row = vec![
                        skill.effect.order,
                        skill.effect.kind,
                        skill.ability_id,
                        skill.effect.img1,
                        skill.effect.img2,
                        skill.effect.content,
                    ];

                    ctx.roguelike_effect_rows.insert(ability, row);
                } else {
                    let row = ctx.picture_book_abilities[ability as usize].clone();
                    let entry = ctx.roguelike_effect_rows.entry(ability).or_default();

                    *entry = row;
                    *entry.first_mut().ok_or(Fault::index_out_of_range(0, 0))? = skill.effect.order;
                }
            }
        }
    }

    Ok(ctx.roguelike_effect_rows.clone())
}
