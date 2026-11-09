use std::{collections::BTreeMap, rc::Rc};

use crate::Fault;

use super::{
    JsonNode, RoguelikeBuff, RoguelikeBuffSkill, RoguelikeEffect, json_container_as_int,
    json_container_as_string, json_string_as_int, json_string_as_string, json_value_as_int,
    json_value_as_string,
};

pub fn parse_roguelike_bonus_buff(
    record: &mut RoguelikeBuff,
    skills: &BTreeMap<i32, Rc<RoguelikeBuffSkill>>,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    if !matches!(fields.get(b"Rarity".as_slice()), Some(JsonNode::Uint(_))) {
        return Ok(false);
    }

    record.rarity = match fields.get(b"Rarity".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;

    if !matches!(
        fields.get(b"RoguelikeBonusBuffName".as_slice()),
        Some(JsonNode::String(_))
    ) {
        return Ok(false);
    }

    record.name = match fields.get(b"RoguelikeBonusBuffName".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_string(text),
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_string(),
        Some(found) => json_value_as_string(found),
        None => Vec::new(),
    };

    if !matches!(
        fields.get(b"RoguelikeBonusBuffExplanation".as_slice()),
        Some(JsonNode::String(_))
    ) {
        return Ok(false);
    }

    record.explanation = match fields.get(b"RoguelikeBonusBuffExplanation".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_string(text),
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_string(),
        Some(found) => json_value_as_string(found),
        None => Vec::new(),
    };

    if !matches!(fields.get(b"BuffType".as_slice()), Some(JsonNode::Uint(_))) {
        return Ok(false);
    }

    let buff_type = match fields.get(b"BuffType".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;

    match buff_type as u32 {
        0 => {
            let plus = match fields.get(b"PlusUp".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;
            let level = match fields.get(b"LevelUp".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;

            record
                .effects
                .push(Some(RoguelikeEffect::PowerUp { level, plus }));
        }
        1 => {
            if !matches!(
                fields.get(b"BuffSkillID".as_slice()),
                Some(JsonNode::Uint(_))
            ) {
                return Ok(false);
            }

            let skill_id = match fields.get(b"BuffSkillID".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;
            let Some(skill) = skills.get(&skill_id) else {
                return Ok(false);
            };

            record
                .effects
                .push(Some(RoguelikeEffect::Instinct(Rc::clone(skill))));
        }
        2 => {
            if !matches!(fields.get(b"EffectID".as_slice()), Some(JsonNode::Uint(_))) {
                return Ok(false);
            }

            let effect = match fields.get(b"EffectID".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;

            if !matches!(fields.get(b"VolumeID".as_slice()), Some(JsonNode::Uint(_))) {
                return Ok(false);
            }

            let band = match fields.get(b"VolumeID".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;

            record
                .effects
                .push(Some(RoguelikeEffect::Combo { effect, band }));
        }
        3 => {
            if !matches!(fields.get(b"ID".as_slice()), Some(JsonNode::Uint(_))) {
                return Ok(false);
            }

            let cannon = match fields.get(b"ID".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;

            if !matches!(fields.get(b"Level".as_slice()), Some(JsonNode::Uint(_))) {
                return Ok(false);
            }

            let level = match fields.get(b"Level".as_slice()) {
                Some(JsonNode::String(text)) => json_string_as_int(text)?,
                Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
                Some(found) => json_value_as_int(found),
                None => 0,
            } as i32;

            record
                .effects
                .push(Some(RoguelikeEffect::CatCannon { cannon, level }));
        }
        _ => {}
    }

    record.limit = match fields.get(b"Limit".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0x63,
    } as i32;
    record.target_chara = match fields.get(b"TargetCharaID".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => -1,
    } as i32;
    record.target_rarity = match fields.get(b"TargetRarity".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => -1,
    } as i32;

    Ok(true)
}
