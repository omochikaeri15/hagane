use std::rc::Rc;

use crate::Fault;

use super::{
    AppContext, RoguelikeEffect, get_cat_name, query_localizable, string_format_int,
    substitute_tokens,
};

pub fn roguelike_bonus_explanation(
    ctx: &mut AppContext,
    buff_id: i32,
    form: i32,
) -> Result<Vec<u8>, Fault> {
    let master = Rc::clone(&ctx.roguelike.master);
    let Some(buff) = master.buffs.get(&buff_id) else {
        return Ok(Vec::new());
    };
    let Some(Some(effect)) = buff.effects.first() else {
        return Ok(Vec::new());
    };

    let mut chara_name: Vec<u8> = Vec::new();
    let mut rarity_name: Vec<u8> = Vec::new();
    let mut skill_name: Vec<u8> = Vec::new();
    let mut bonus_value: Vec<u8> = Vec::new();
    let mut allow_duplicates: Vec<u8> = Vec::new();

    if buff.target_chara != -1 {
        chara_name = get_cat_name(ctx, buff.target_chara, form);
    }

    if buff.target_rarity != -1 {
        let key = string_format_int(ctx, b"rarity_name_%d", buff.target_rarity.wrapping_add(1))?;

        rarity_name = query_localizable(ctx, &key);
    }

    let kind = effect.kind();

    if kind == 1 {
        let key = string_format_int(ctx, b"potential_skill_name%02d", effect.first())?;

        skill_name = query_localizable(ctx, &key);

        let RoguelikeEffect::Instinct(skill) = effect else {
            return Err(Fault::null_pointer());
        };

        bonus_value = match skill.params.first() {
            Some(value) => value.to_string().into_bytes(),
            None => Vec::new(),
        };
        allow_duplicates = if effect.second() != 1 {
            query_localizable(ctx, b"RoguelikeBonusAllowDuplicates_true")
        } else {
            query_localizable(ctx, b"RoguelikeBonusAllowDuplicates_false")
        };
    } else if kind == 0 {
        bonus_value = effect.second().wrapping_add(effect.first()).to_string().into_bytes();
    }

    let text = query_localizable(ctx, &buff.explanation);

    substitute_tokens(
        ctx,
        &text,
        &[
            (b"skillName", &skill_name),
            (b"bonusValue", &bonus_value),
            (b"targetRarity", &rarity_name),
            (b"charaName", &chara_name),
            (b"allowDuplicates", &allow_duplicates),
        ],
    )
}
