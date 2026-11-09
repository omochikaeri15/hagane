use crate::Fault;

use super::{
    AppContext, JsonNode, RoguelikeMapEntry, json_container_as_int, json_container_as_string,
    json_parse_object_document, json_source_from_string, json_string_as_int, json_string_as_string,
    json_value_as_int, json_value_as_string, open_asset_stream, string_to_int,
};

pub fn load_roguelike_map_settings_json(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.roguelike.settings.clear();

    let Some(bytes) = open_asset_stream(ctx, b"RoguelikeMapSettings.json", 0, 0)? else {
        return Ok(());
    };

    let source = json_source_from_string(&bytes);
    let document = json_parse_object_document(Some(source))?;
    let Some(JsonNode::Object(root)) = document.as_ref() else {
        return Ok(());
    };
    let Some(JsonNode::Object(events)) = root.get(b"EventID".as_slice()) else {
        return Ok(());
    };

    for (event_key, entry) in events {
        let event_id = string_to_int(event_key)?;

        ctx.roguelike.settings.entry(event_id).or_default();

        let JsonNode::Object(fields) = entry else {
            return Err(Fault::null_pointer());
        };

        if let Some(JsonNode::Array(rows)) = fields.get(b"Data".as_slice()) {
            for row in rows {
                ctx.roguelike
                    .settings
                    .entry(event_id)
                    .or_default()
                    .entries
                    .push(RoguelikeMapEntry::default());

                let JsonNode::Object(cells) = row else {
                    return Err(Fault::null_pointer());
                };

                if let Some(JsonNode::Array(values)) = cells.get(b"MapID".as_slice()) {
                    for found in values {
                        let value = match found {
                            JsonNode::String(text) => json_string_as_int(text)?,
                            JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                            _ => json_value_as_int(found),
                        } as i32;

                        ctx.roguelike
                            .settings
                            .entry(event_id)
                            .or_default()
                            .entries
                            .last_mut()
                            .ok_or(Fault::null_pointer())?
                            .map_ids
                            .push(value);
                    }
                }

                if let Some(JsonNode::Array(values)) = cells.get(b"Level".as_slice()) {
                    for found in values {
                        let value = match found {
                            JsonNode::String(text) => json_string_as_int(text)?,
                            JsonNode::Array(_) | JsonNode::Object(_) => json_container_as_int(),
                            _ => json_value_as_int(found),
                        } as i32;

                        ctx.roguelike
                            .settings
                            .entry(event_id)
                            .or_default()
                            .entries
                            .last_mut()
                            .ok_or(Fault::null_pointer())?
                            .levels
                            .push(value);
                    }
                }

                let preset = match cells.get(b"PresetFileName".as_slice()) {
                    Some(JsonNode::String(text)) => json_string_as_string(text),
                    Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => {
                        json_container_as_string()
                    }
                    Some(found) => json_value_as_string(found),
                    None => Vec::new(),
                };

                ctx.roguelike
                    .settings
                    .entry(event_id)
                    .or_default()
                    .entries
                    .last_mut()
                    .ok_or(Fault::null_pointer())?
                    .preset_file_name = preset;
            }
        }

        let reset = match fields.get(b"ResetMapID".as_slice()) {
            Some(JsonNode::String(text)) => json_string_as_int(text)?,
            Some(JsonNode::Array(_)) | Some(JsonNode::Object(_)) => json_container_as_int(),
            Some(found) => json_value_as_int(found),
            None => 0,
        } as i32;

        ctx.roguelike
            .settings
            .entry(event_id)
            .or_default()
            .reset_map_id = reset;
    }

    Ok(())
}
