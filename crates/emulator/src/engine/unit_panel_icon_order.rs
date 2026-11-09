use crate::Fault;

use super::{AppContext, unit_panel_icon_table};

pub fn unit_panel_icon_order(ctx: &mut AppContext) -> Result<Vec<(i32, i32)>, Fault> {
    let mut order: Vec<(i32, i32)> = Vec::new();
    let table = unit_panel_icon_table(ctx)?;

    for (index, row) in &table {
        let rank = *row.first().ok_or(Fault::null_pointer())?;

        order.push((*index, rank));
    }

    order.sort_by_key(|pair| pair.1);

    Ok(order)
}
