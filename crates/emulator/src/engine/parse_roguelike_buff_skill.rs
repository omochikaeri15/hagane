use std::collections::BTreeMap;

use crate::Fault;

use super::{
    JsonNode, RoguelikeBuffSkill, RoguelikeEffectAbility, json_container_as_int,
    json_string_as_int, json_value_as_int,
};

pub fn parse_roguelike_buff_skill(
    record: &mut RoguelikeBuffSkill,
    effects: &BTreeMap<i32, RoguelikeEffectAbility>,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    if !matches!(fields.get(b"AbilityID".as_slice()), Some(JsonNode::Uint(_))) {
        return Ok(false);
    }

    record.ability_id = match fields.get(b"AbilityID".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;

    let Some(effect) = effects.get(&record.ability_id) else {
        return Ok(false);
    };

    record.effect = *effect;

    if !matches!(fields.get(b"MaxLevel".as_slice()), Some(JsonNode::Uint(_))) {
        return Ok(false);
    }

    record.max_level = match fields.get(b"MaxLevel".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;

    if let Some(JsonNode::Array(values)) = fields.get(b"Parameters".as_slice()) {
        for found in values {
            let value = match found {
                JsonNode::String(text) => json_string_as_int(text)?,
                JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                _ => json_value_as_int(found),
            } as i32;

            record.params.push(value);
        }
    }

    Ok(true)
}
