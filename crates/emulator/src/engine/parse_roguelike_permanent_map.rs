use std::collections::BTreeMap;

use crate::Fault;

use super::{JsonNode, parse_roguelike_permanent_groups, string_to_int};

pub fn parse_roguelike_permanent_map(
    crowns: &mut BTreeMap<i32, Vec<i32>>,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    for (key, value) in fields {
        let crown = string_to_int(key)?;
        let JsonNode::Object(cells) = value else {
            return Ok(false);
        };

        if !parse_roguelike_permanent_groups(crowns.entry(crown).or_default(), cells)? {
            return Ok(false);
        }
    }

    Ok(true)
}
