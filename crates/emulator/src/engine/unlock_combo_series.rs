use crate::Fault;

use super::AppContext;

pub fn unlock_combo_series(ctx: &mut AppContext, series: i32) -> Result<i32, Fault> {
    if series != 7 {
        let mut definition = 0usize;

        while definition < ctx.combo_definitions.len() {
            let row = &ctx.combo_definitions[definition];
            let limit = row.len() as i64;

            if *row.get(1).ok_or(Fault::index_out_of_range(1, limit))? == series {
                let limit = ctx.combo_store.states.len() as i64;
                let state = ctx
                    .combo_store
                    .states
                    .get_mut(definition)
                    .ok_or(Fault::index_out_of_range(definition as i64, limit))?;

                if *state == 2 {
                    *state = 1;
                }
            }

            definition += 1;
        }

        return Ok(0);
    }

    let mut unlocked: Vec<i32> = Vec::new();
    let units: Vec<i32> = ctx.talent_definitions.keys().copied().collect();

    for unit_id in units {
        let mut talent_slot = 0usize;

        loop {
            if ctx.talent_definitions.get(&unit_id).ok_or(Fault::null_pointer())?[talent_slot * 0xe + 1] == 0x49 {
                break;
            }

            talent_slot += 1;

            if talent_slot == ctx.limits.talent_groups as usize {
                break;
            }
        }

        if talent_slot == ctx.limits.talent_groups as usize {
            continue;
        }

        let levels = ctx.talent_levels.entry(unit_id).or_default();
        let definition = ctx.talent_definitions.get(&unit_id).ok_or(Fault::null_pointer())?;
        let level = *levels.entry(definition[talent_slot * 0xe + 1]).or_default();

        if level > 0 {
            unlocked.push(definition[talent_slot * 0xe + 3]);
        }
    }

    let mut definition = 0usize;

    while definition < ctx.combo_definitions.len() {
        let row = &ctx.combo_definitions[definition];
        let limit = row.len() as i64;

        if *row.get(1).ok_or(Fault::index_out_of_range(1, limit))? == 7 {
            let limit = ctx.combo_store.states.len() as i64;
            let state = ctx
                .combo_store
                .states
                .get_mut(definition)
                .ok_or(Fault::index_out_of_range(definition as i64, limit))?;

            if *state == 2 && unlocked.contains(&(definition as i32)) {
                *state = 1;
            }
        }

        definition += 1;
    }

    Ok(0)
}
