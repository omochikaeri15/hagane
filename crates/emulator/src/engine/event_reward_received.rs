use crate::Fault;

use super::{AppContext, is_cap_extension_map};

pub fn event_reward_received(
    ctx: &mut AppContext,
    event: i32,
    stage: i32,
    crown: i32,
    kind: i32,
    use_cache: i32,
) -> Result<bool, Fault> {
    let mut map_id = -1i32;

    for (listed, reward) in &ctx.event_reward_maps {
        if *reward == event {
            map_id = *listed;

            break;
        }
    }

    if map_id == -1 {
        return Ok(false);
    }

    let slot = if kind == 2 { crown } else { 0 };

    if use_cache != 0 {
        if !ctx.event_reward_cache.contains_key(&event) {
            return Ok(false);
        }

        if !ctx
            .event_reward_cache
            .entry(event)
            .or_default()
            .contains_key(&stage)
        {
            return Ok(false);
        }

        let cell = ctx
            .event_reward_cache
            .entry(event)
            .or_default()
            .entry(stage)
            .or_default();

        return Ok(*cell
            .get(slot as i64 as usize)
            .ok_or(Fault::index_out_of_range(slot as i64, 4))?
            != 0);
    }

    if event == -2 {
        let cell = (stage as i64) * 4 + slot as i64 + AppContext::EVENT_REWARDS_NEG2 as i64;

        return Ok(ctx.u8_at(cell as usize)? != 0);
    }

    if is_cap_extension_map(&ctx.map_options, map_id) {
        return Ok(ctx
            .keyed_event_rewards
            .get(&(event, stage, slot))
            .is_some_and(|flag| *flag != 0));
    }

    let cell =
        (event as i64) * 0x30 + (stage as i64) * 4 + slot as i64 + AppContext::EVENT_REWARDS as i64;

    Ok(ctx.u8_at(cell as usize)? != 0)
}
