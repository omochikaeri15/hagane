use crate::Fault;

use super::{AppContext, is_cap_extension_map};

pub fn set_ranking_best_score(
    ctx: &mut AppContext,
    ranking: i32,
    stage: i32,
    crown: i32,
    score: i32,
) -> Result<(), Fault> {
    let mut map_id = -1i32;

    for (listed, id) in &ctx.ranking_maps {
        if *id == ranking {
            map_id = *listed;

            break;
        }
    }

    if map_id == -1 {
        return Ok(());
    }

    if is_cap_extension_map(&ctx.map_options, map_id) {
        *ctx.keyed_ranking_scores.entry((ranking, stage, crown)).or_insert(0) = score;

        return Ok(());
    }

    let cell = (ranking as i64) * 0xf0
        + (stage as i64) * 0x10
        + (crown as i64) * 4
        + AppContext::RANKING_BEST_SCORES as i64;

    ctx.set_i32_at(cell as usize, score)
}
