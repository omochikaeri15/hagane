use std::collections::BTreeMap;

use crate::Fault;

use super::{
    AbilityRow, AppContext, FixedLineupUnit, JsonNode, LineupPreset, get_unit_form_count, get_unit_guide_order,
    get_unit_max_level, get_unit_max_plus_level, is_unit_available, json_container_as_bool,
    json_container_as_int, json_string_as_bool, json_string_as_int, json_value_as_bool,
    json_value_as_int, string_to_int,
};

pub fn parse_lineup_preset(
    ctx: &mut AppContext,
    preset: &mut LineupPreset,
    root: Option<&JsonNode>,
) -> Result<bool, Fault> {
    let Some(root) = root else {
        return Ok(false);
    };
    let JsonNode::Object(root) = root else {
        return Err(Fault::null_pointer());
    };
    let slot = match root.get(b"slot".as_slice()) {
        Some(JsonNode::Object(slot)) => Some(slot),
        _ => None,
    };
    let mut unit_index: BTreeMap<i32, i32> = BTreeMap::new();

    if let Some(slot) = slot
        && let Some(JsonNode::Object(_)) = slot.get(b"data".as_slice())
    {
        let Some(JsonNode::Object(data)) = slot.get(b"data".as_slice()) else {
            return Err(Fault::null_pointer());
        };

        if let Some((_, entry)) = data.iter().next() {
            let JsonNode::Object(entry) = entry else {
                return Err(Fault::null_pointer());
            };
            let mut index = 0usize;

            loop {
                let Some(JsonNode::Array(chara)) = entry.get(b"chara".as_slice()) else {
                    return Err(Fault::null_pointer());
                };

                if index >= chara.len() {
                    break;
                }

                preset.units
                    .push(FixedLineupUnit::default());

                let node = &chara[index];
                let unit_id = match node {
                    JsonNode::String(text) => json_string_as_int(text)?,
                    JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                    _ => json_value_as_int(node),
                } as i32;
                let unit = preset.units
                    .last_mut()
                    .ok_or(Fault::null_pointer())?;

                unit.unit_id = unit_id;
                unit_index.insert(unit_id, index as i32);
                index += 1;
            }

            match entry.get(b"cannon".as_slice()) {
                Some(node) => {
                    let cannon = match node {
                        JsonNode::String(text) => json_string_as_int(text)?,
                        JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                        _ => json_value_as_int(node),
                    } as i32;

                    preset.cannon_type = cannon;
                }
                None => preset.cannon_type = -1,
            }
        }
    }

    if let Some(JsonNode::Object(chara)) = root.get(b"chara".as_slice())
        && let Some(JsonNode::Object(_)) = chara.get(b"data".as_slice())
    {
        let units: Vec<i32> = unit_index.keys().copied().collect();

        for unit_id in units {
            let Some(JsonNode::Object(data)) = chara.get(b"data".as_slice()) else {
                return Err(Fault::null_pointer());
            };
            let Some(JsonNode::Object(entry)) = data.get(unit_id.to_string().as_bytes()) else {
                return Ok(false);
            };

            if unit_id as u32 >= ctx.limits.units as u32
                || !is_unit_available(ctx, unit_id)?
                || get_unit_guide_order(ctx, unit_id)? == -1
            {
                return Ok(false);
            }

            if let Some(node) = entry.get(b"evolution".as_slice()) {
                let evolution = match node {
                    JsonNode::String(text) => json_string_as_int(text)?,
                    JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                    _ => json_value_as_int(node),
                } as i32;

                if evolution <= 0 || evolution > get_unit_form_count(ctx, unit_id)? {
                    return Ok(false);
                }

                let index = *unit_index.entry(unit_id).or_insert(0);

                preset.units
                    .get_mut(index as usize)
                    .ok_or(Fault::index_out_of_range(index as i64, 0))?
                    .form = evolution.wrapping_sub(1);
            }

            if let Some(node) = entry.get(b"level".as_slice()) {
                let level = match node {
                    JsonNode::String(text) => json_string_as_int(text)?,
                    JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                    _ => json_value_as_int(node),
                } as i32;

                if level <= 0 || level > get_unit_max_level(ctx, unit_id)? {
                    return Ok(false);
                }

                let index = *unit_index.entry(unit_id).or_insert(0);

                preset.units
                    .get_mut(index as usize)
                    .ok_or(Fault::index_out_of_range(index as i64, 0))?
                    .level = level.wrapping_sub(1);
            }

            if let Some(node) = entry.get(b"plus".as_slice()) {
                let plus = match node {
                    JsonNode::String(text) => json_string_as_int(text)?,
                    JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                    _ => json_value_as_int(node),
                } as i32;

                if plus < 0 || plus > get_unit_max_plus_level(ctx, unit_id)? {
                    return Ok(false);
                }

                let index = *unit_index.entry(unit_id).or_insert(0);

                preset.units
                    .get_mut(index as usize)
                    .ok_or(Fault::index_out_of_range(index as i64, 0))?
                    .plus_level = plus;
            }
        }
    }

    if let Some(JsonNode::Object(ability)) = root.get(b"ability".as_slice()) {
        let Some(JsonNode::Object(data)) = ability.get(b"data".as_slice()) else {
            return Err(Fault::null_pointer());
        };

        for (key, value) in data {
            let kind = string_to_int(key)?;

            if kind as u32 > 9 {
                return Ok(false);
            }

            let JsonNode::Object(value) = value else {
                return Err(Fault::null_pointer());
            };
            let level = value
                .get(b"level".as_slice())
                .map_or(Ok(0), |node| match node {
                    JsonNode::String(text) => json_string_as_int(text),
                    JsonNode::Array(_) | JsonNode::Object(_) => Ok(json_container_as_int()),
                    _ => Ok(json_value_as_int(node)),
                })? as i32;
            let plus = value
                .get(b"plus".as_slice())
                .map_or(Ok(0), |node| match node {
                    JsonNode::String(text) => json_string_as_int(text),
                    JsonNode::Array(_) | JsonNode::Object(_) => Ok(json_container_as_int()),
                    _ => Ok(json_value_as_int(node)),
                })? as i32;

            if level <= 0 || plus < 0 {
                return Ok(false);
            }

            let row = (kind as usize)
                .wrapping_mul(AbilityRow::STRIDE)
                .wrapping_add(AppContext::ABILITY_DATA);

            if level > ctx.i32_at(row + AbilityRow::LEVEL_CAP)? || plus > ctx.i32_at(row + AbilityRow::PLUS_CAP)? {
                return Ok(false);
            }

            preset.ability_levels
                .insert(kind, level.wrapping_add(plus).wrapping_sub(1));
        }
    }

    if let Some(JsonNode::Object(cannon)) = root.get(b"cannon".as_slice()) {
        let Some(JsonNode::Object(data)) = cannon.get(b"data".as_slice()) else {
            return Err(Fault::null_pointer());
        };

        for (key, value) in data {
            let part = string_to_int(key)?;
            let JsonNode::Object(value) = value else {
                return Err(Fault::null_pointer());
            };
            let level = (value
                .get(b"level".as_slice())
                .map_or(Ok(0), |node| match node {
                    JsonNode::String(text) => json_string_as_int(text),
                    JsonNode::Array(_) | JsonNode::Object(_) => Ok(json_container_as_int()),
                    _ => Ok(json_value_as_int(node)),
                })? as i32)
                .wrapping_sub(1);

            if part == 0 {
                preset.base_level = level;
                preset.cannon_level = level;
            } else {
                let cannon_type = preset.cannon_type;

                if cannon_type != -1 && part == cannon_type {
                    preset.cannon_level = level;
                }
            }
        }
    }

    let Some(JsonNode::Object(treasure)) = root.get(b"treasure".as_slice()) else {
        return Ok(true);
    };

    if let Some(JsonNode::Object(_)) = treasure.get(b"defaultData".as_slice()) {
        let Some(JsonNode::Object(default)) = treasure.get(b"defaultData".as_slice()) else {
            return Err(Fault::null_pointer());
        };
        let none = default
            .get(b"none".as_slice())
            .is_some_and(|node| match node {
                JsonNode::String(text) => json_string_as_bool(text),
                JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_bool(),
                _ => json_value_as_bool(node),
            });

        if none {
            for chapter in 0..10 {
                preset.treasure_values.insert(chapter, 0);
            }
        }
    }

    let Some(JsonNode::Object(data)) = treasure.get(b"data".as_slice()) else {
        return Ok(true);
    };

    for (key, value) in data {
        let chapter = string_to_int(key)?;

        if chapter as u32 > 9 || chapter == 3 {
            return Ok(false);
        }

        let JsonNode::Object(value) = value else {
            return Err(Fault::null_pointer());
        };
        let Some(JsonNode::Array(count)) = value.get(b"count".as_slice()) else {
            return Err(Fault::null_pointer());
        };
        let mut value = 0i32;

        if count.len() * 8 >= 0x11 {
            let mut cell = 2usize;

            loop {
                let found = match &count[cell] {
                    JsonNode::String(text) => json_string_as_int(text)?,
                    JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                    node => json_value_as_int(node),
                };

                if found == 0x30 {
                    value = [0x21, 0x42, 0x64][cell];
                    break;
                }

                if cell == 0 {
                    break;
                }

                cell -= 1;
            }
        }

        preset.treasure_values.insert(chapter, value);
    }

    Ok(true)
}
