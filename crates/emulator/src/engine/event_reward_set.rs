use crate::Fault;

use super::{AppContext, is_cap_extension_map};

pub fn event_reward_set(
    ctx: &mut AppContext,
    event: i32,
    stage: i32,
    crown: i32,
    kind: i32,
    value: u8,
    use_cache: i32,
) -> Result<(), Fault> {
    let mut map_id = -1i32;

    for (listed, reward) in &ctx.event_reward_maps {
        if *reward == event {
            map_id = *listed;

            break;
        }
    }

    if map_id == -1 {
        return Ok(());
    }

    let slot = if kind == 2 { crown } else { 0 };

    if use_cache != 0 {
        let cell = ctx
            .event_reward_cache
            .entry(event)
            .or_default()
            .entry(stage)
            .or_default();
        let byte = cell
            .get_mut(slot as i64 as usize)
            .ok_or(Fault::index_out_of_range(slot as i64, 4))?;

        *byte = value;

        return Ok(());
    }

    if event == -2 {
        let cell = (stage as i64) * 4 + slot as i64 + AppContext::EVENT_REWARDS_NEG2 as i64;

        return ctx.set_block_at::<1>(cell as usize, [value]);
    }

    if is_cap_extension_map(&ctx.map_options, map_id) {
        *ctx.keyed_event_rewards.entry((event, stage, slot)).or_insert(0) = value;

        return Ok(());
    }

    let cell =
        (event as i64) * 0x30 + (stage as i64) * 4 + slot as i64 + AppContext::EVENT_REWARDS as i64;

    ctx.set_block_at::<1>(cell as usize, [value])
}
