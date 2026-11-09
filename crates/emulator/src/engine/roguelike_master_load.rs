use crate::Fault;

use super::{
    AppContext, load_roguelike_bonus_buff_json, load_roguelike_bonus_chara_json,
    load_roguelike_buff_skill_json, load_roguelike_effect_ability_json,
    load_roguelike_lottery_group_json, load_roguelike_permanent_json,
};

pub fn roguelike_master_load(ctx: &mut AppContext) -> Result<bool, Fault> {
    let mut loaded = load_roguelike_bonus_chara_json(ctx)?;

    if !load_roguelike_effect_ability_json(ctx)? {
        loaded = false;
    }

    if !load_roguelike_buff_skill_json(ctx)? {
        loaded = false;
    }

    if !load_roguelike_bonus_buff_json(ctx)? {
        loaded = false;
    }

    if !load_roguelike_lottery_group_json(ctx)? {
        loaded = false;
    }

    if !load_roguelike_permanent_json(ctx)? {
        loaded = false;
    }

    Ok(loaded)
}
