use std::collections::BTreeMap;

use crate::Fault;

use super::{JsonNode, json_container_as_int, json_string_as_int, json_value_as_int};

pub fn parse_roguelike_permanent_groups(
    groups: &mut Vec<i32>,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    let Some(JsonNode::Array(values)) = fields.get(b"BonusGroupID".as_slice()) else {
        return Ok(false);
    };

    for found in values {
        let value = match found {
            JsonNode::String(text) => json_string_as_int(text)?,
            JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
            _ => json_value_as_int(found),
        } as i32;

        groups.push(value);
    }

    Ok(true)
}
