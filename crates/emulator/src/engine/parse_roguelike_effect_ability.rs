use std::collections::BTreeMap;

use crate::Fault;

use super::{
    JsonNode, RoguelikeEffectAbility, json_container_as_int, json_string_as_int, json_value_as_int,
};

pub fn parse_roguelike_effect_ability(
    record: &mut RoguelikeEffectAbility,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    if !matches!(fields.get(b"Order".as_slice()), Some(JsonNode::Uint(_))) {
        return Ok(false);
    }

    record.order = match fields.get(b"Order".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;
    record.kind = match fields.get(b"Kind".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => -1,
    } as i32;
    record.ability_id = match fields.get(b"AbilityID".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;
    record.img1 = match fields.get(b"Img1".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => -1,
    } as i32;
    record.img2 = match fields.get(b"Img2".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => -1,
    } as i32;
    record.content = match fields.get(b"Content".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => -1,
    } as i32;

    Ok(true)
}
