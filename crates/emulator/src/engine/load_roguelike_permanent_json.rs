use std::rc::Rc;

use crate::Fault;

use super::{
    AppContext, JsonNode, json_parse_object_document, json_source_from_string, open_asset_stream,
    parse_roguelike_permanent_event, string_to_int,
};

pub fn load_roguelike_permanent_json(ctx: &mut AppContext) -> Result<bool, Fault> {
    let Some(bytes) = open_asset_stream(ctx, b"RoguelikeBonusPermanent.json", 0, 0)? else {
        return Ok(false);
    };

    let source = json_source_from_string(&bytes);
    let document = json_parse_object_document(Some(source))?;
    let Some(JsonNode::Object(root)) = document.as_ref() else {
        return Ok(false);
    };
    let Some(JsonNode::Object(entries)) = root.get(b"EventID".as_slice()) else {
        return Ok(false);
    };

    for (key, value) in entries {
        let id = string_to_int(key)?;
        let JsonNode::Object(fields) = value else {
            return Ok(false);
        };
        let master = Rc::make_mut(&mut ctx.roguelike.master);
        let record = master.permanent.entry(id).or_default();

        if !parse_roguelike_permanent_event(record, fields)? {
            return Ok(false);
        }
    }

    Ok(true)
}
