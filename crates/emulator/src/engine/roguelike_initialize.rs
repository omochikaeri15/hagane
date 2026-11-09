use crate::Fault;

use super::{AppContext, load_roguelike_map_settings_json, roguelike_master_load};

pub fn roguelike_initialize(ctx: &mut AppContext) -> Result<(), Fault> {
    load_roguelike_map_settings_json(ctx)?;
    roguelike_master_load(ctx)?;

    Ok(())
}
