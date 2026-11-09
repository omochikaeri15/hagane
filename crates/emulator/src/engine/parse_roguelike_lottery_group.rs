use std::collections::BTreeMap;

use crate::Fault;

use super::{
    JsonNode, RoguelikeLotteryGroup, json_container_as_int, json_string_as_int, json_value_as_int,
};

pub fn parse_roguelike_lottery_group(
    record: &mut RoguelikeLotteryGroup,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    if !matches!(
        fields.get(b"BonusTarget".as_slice()),
        Some(JsonNode::Uint(_))
    ) {
        return Ok(false);
    }

    record.target = match fields.get(b"BonusTarget".as_slice()) {
        Some(JsonNode::String(text)) => json_string_as_int(text)?,
        Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
        Some(found) => json_value_as_int(found),
        None => 0,
    } as i32;

    let Some(JsonNode::Array(values)) = fields.get(b"BonusTargetID".as_slice()) else {
        return Ok(false);
    };

    for found in values {
        let value = match found {
            JsonNode::String(text) => json_string_as_int(text)?,
            JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
            _ => json_value_as_int(found),
        } as i32;

        record.target_ids.push(value);
    }

    Ok(true)
}
