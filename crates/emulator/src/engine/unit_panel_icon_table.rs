use std::collections::BTreeMap;

use crate::Fault;

use super::{AppContext, is_roguelike_map, roguelike_effect_table};

pub fn unit_panel_icon_table(ctx: &mut AppContext) -> Result<BTreeMap<i32, Vec<i32>>, Fault> {
    if is_roguelike_map(ctx, -1, -1)? {
        return roguelike_effect_table(ctx);
    }

    let mut table: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    let mut index = 0usize;

    while index < ctx.picture_book_abilities.len() {
        table.insert(index as i32, ctx.picture_book_abilities[index].clone());
        index += 1;
    }

    Ok(table)
}
