use std::rc::Rc;

use crate::Fault;

use super::{
    AppContext, JsonNode, RoguelikeBuff, json_parse_object_document, json_source_from_string,
    open_asset_stream, parse_roguelike_bonus_buff, string_to_int,
};

pub fn load_roguelike_bonus_buff_json(ctx: &mut AppContext) -> Result<bool, Fault> {
    let Some(bytes) = open_asset_stream(ctx, b"RoguelikeBonusBuff.json", 0, 0)? else {
        return Ok(false);
    };

    let source = json_source_from_string(&bytes);
    let document = json_parse_object_document(Some(source))?;
    let Some(JsonNode::Object(root)) = document.as_ref() else {
        return Ok(false);
    };
    let Some(JsonNode::Object(entries)) = root.get(b"BonusBuffID".as_slice()) else {
        return Ok(false);
    };

    for (key, value) in entries {
        let id = string_to_int(key)?;
        let JsonNode::Object(fields) = value else {
            return Ok(false);
        };
        let master = Rc::make_mut(&mut ctx.roguelike.master);
        let mut record = master.buffs.remove(&id).unwrap_or_else(|| RoguelikeBuff {
            id,
            ..Default::default()
        });
        let parsed = parse_roguelike_bonus_buff(&mut record, &master.skills, fields)?;

        master.buffs.insert(id, record);

        if !parsed {
            return Ok(false);
        }
    }

    Ok(true)
}
