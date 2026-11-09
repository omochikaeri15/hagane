use crate::Fault;

use super::{EventItemStore, find_point_rule_entry, get_point_rule_type};

pub fn has_point_rule(store: Option<&EventItemStore>, rule_id: i32) -> Result<bool, Fault> {
    let Some(store) = store else {
        return Ok(false);
    };

    let table = store.rules.as_ref().ok_or(Fault::null_pointer())?;
    let rule_id = if rule_id == -1 { store.rule_id } else { rule_id };
    let entry = find_point_rule_entry(table, rule_id)?.ok_or(Fault::null_pointer())?;

    Ok(get_point_rule_type(entry) != -1)
}
