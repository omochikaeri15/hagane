use std::mem;

use crate::Fault;

use super::{
    AppContext, LineupPreset, json_parse_object_document, json_source_from_string,
    open_asset_stream, parse_lineup_preset,
};

pub fn load_lineup_preset(ctx: &mut AppContext, roguelike: bool, name: &[u8]) -> Result<(), Fault> {
    let mut preset = if roguelike {
        mem::take(&mut ctx.roguelike.lineup)
    } else {
        LineupPreset {
            units: mem::take(&mut ctx.fixed_lineup_store.units),
            ability_levels: mem::take(&mut ctx.fixed_lineup_store.ability_levels),
            cannon_type: ctx.i32_at(AppContext::LINEUP_CANNON_TYPE)?,
            cannon_level: ctx.i32_at(AppContext::LINEUP_CANNON_LEVEL)?,
            base_level: ctx.i32_at(AppContext::LINEUP_BASE_LEVEL)?,
            treasure_values: mem::take(&mut ctx.fixed_lineup_store.treasure_values),
        }
    };

    preset.units.clear();
    preset.ability_levels.clear();
    preset.base_level = -1;
    preset.cannon_type = -1;
    preset.cannon_level = -1;
    preset.treasure_values.clear();

    let mut parsed = Ok(false);

    if let Some(bytes) = open_asset_stream(ctx, name, 0, 0)? {
        let source = json_source_from_string(&bytes);

        parsed = match json_parse_object_document(Some(source)) {
            Ok(root) => parse_lineup_preset(ctx, &mut preset, root.as_ref()),
            Err(fault) => Err(fault),
        };
    }

    if roguelike {
        ctx.roguelike.lineup = preset;
    } else {
        ctx.set_i32_at(AppContext::LINEUP_CANNON_TYPE, preset.cannon_type)?;
        ctx.set_i32_at(AppContext::LINEUP_CANNON_LEVEL, preset.cannon_level)?;
        ctx.set_i32_at(AppContext::LINEUP_BASE_LEVEL, preset.base_level)?;
        ctx.fixed_lineup_store.units = preset.units;
        ctx.fixed_lineup_store.ability_levels = preset.ability_levels;
        ctx.fixed_lineup_store.treasure_values = preset.treasure_values;
    }

    parsed?;

    Ok(())
}
