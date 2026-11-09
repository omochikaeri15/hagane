use crate::Fault;

use super::{AppContext, is_cap_extension_map};

pub fn get_ranking_best_score(
    ctx: &AppContext,
    ranking: i32,
    stage: i32,
    crown: i32,
) -> Result<i32, Fault> {
    let mut map_id = -1i32;

    for (listed, id) in &ctx.ranking_maps {
        if *id == ranking {
            map_id = *listed;

            break;
        }
    }

    if map_id == -1 {
        return Ok(0);
    }

    if is_cap_extension_map(&ctx.map_options, map_id) {
        return Ok(ctx
            .keyed_ranking_scores
            .get(&(ranking, stage, crown))
            .map_or(0, |score| *score));
    }

    let cell = (ranking as i64) * 0xf0
        + (stage as i64) * 0x10
        + (crown as i64) * 4
        + AppContext::RANKING_BEST_SCORES as i64;

    ctx.i32_at(cell as usize)
}
