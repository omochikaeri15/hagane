use std::collections::BTreeMap;

use crate::Fault;

use super::{JsonNode, parse_roguelike_permanent_map, string_to_int};

pub fn parse_roguelike_permanent_event(
    maps: &mut BTreeMap<i32, BTreeMap<i32, Vec<i32>>>,
    fields: &BTreeMap<Vec<u8>, JsonNode>,
) -> Result<bool, Fault> {
    for (key, value) in fields {
        let map_id = string_to_int(key)?;
        let JsonNode::Object(cells) = value else {
            return Ok(false);
        };

        if !parse_roguelike_permanent_map(maps.entry(map_id).or_default(), cells)? {
            return Ok(false);
        }
    }

    Ok(true)
}
