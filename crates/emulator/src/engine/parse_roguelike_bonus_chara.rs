use std::collections::BTreeMap;

use crate::Fault;

use super::{
    JsonNode, RoguelikeCharaBonus, json_container_as_int, json_string_as_int, json_value_as_int,
};

pub fn parse_roguelike_bonus_chara(
    record: &mut RoguelikeCharaBonus,
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
    record.chara_id = match fields.get(b"CharaID".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;
    record.level = match fields.get(b"Level".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;
    record.plus_level = match fields.get(b"PlusLevel".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;

    Ok(true)
}
