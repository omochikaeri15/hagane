use serde::{Deserialize, Serialize};

use crate::chapter::Category;
use crate::chapter::map::{ExGroup, ExLottery};

use super::Battleground;

const ROLL: i32 = 100;
const WHOLE: f64 = 100.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Continuation {
    pub chance: f64,
    pub category: Category,
    pub map_id: u32,
    pub stage_id: u32,
}

fn add(held: &mut Vec<Continuation>, chance: f64, map: i32, stage: i32) {
    let (Ok(map_id), Ok(stage_id)) = (u32::try_from(map), u32::try_from(stage)) else {
        return;
    };

    if chance <= 0.0 {
        return;
    }

    match held.iter_mut().find(|found| found.map_id == map_id && found.stage_id == stage_id) {
        Some(found) => found.chance += chance,
        None => held.push(Continuation { chance, category: Category::ExtraStages, map_id, stage_id }),
    }
}

pub fn continuations(ground: &Battleground, global_map_id: Option<u32>, stage_id: u32, groups: &ExGroup, lottery: &ExLottery) -> Vec<Continuation> {
    let own = f64::from(ground.ex_chance.clamp(0, ROLL));
    let span = ground.ex_stage_max.wrapping_sub(ground.ex_stage_min).wrapping_add(1).max(1);
    let mut held = Vec::new();

    for step in 0..span {
        add(&mut held, own / f64::from(span), ground.ex_map, ground.ex_stage_min.wrapping_add(step));
    }

    let entry = global_map_id.zip(i32::try_from(stage_id).ok()).and_then(|(map, stage)| groups.entry(i32::try_from(map).ok()?, stage));
    let mut passed = 0;

    for draw in entry.into_iter().flat_map(|entry| &entry.draws) {
        let reach = draw.threshold.clamp(passed, ROLL);
        let share = f64::from(reach - passed) * (WHOLE - own) / WHOLE;

        passed = reach;

        if let Some(target) = usize::try_from(draw.lottery_row).ok().and_then(|row| lottery.entries.get(row)) {
            add(&mut held, share, target.map_index(), target.stage_id);
        }
    }

    held
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::chapter::map::{ExDraw, ExGroupEntry, ExLotteryEntry};

    fn ground(chance: i32, map: i32, low: i32, high: i32) -> Battleground {
        Battleground { ex_chance: chance, ex_map: map, ex_stage_min: low, ex_stage_max: high, ..Battleground::default() }
    }

    fn tables(draws: &[(i32, i32)]) -> (ExGroup, ExLottery) {
        let entry = ExGroupEntry { group: 100, map_id: 24020, stage_id: 10, draws: draws.iter().map(|(lottery_row, threshold)| ExDraw { lottery_row: *lottery_row, threshold: *threshold }).collect() };
        let lottery = ExLottery { entries: vec![ExLotteryEntry { map_id: 4043, stage_id: 0 }, ExLotteryEntry { map_id: 4043, stage_id: 1 }, ExLotteryEntry { map_id: 4044, stage_id: 2 }] };

        (ExGroup { entries: [(ExGroup::key(24020, 10), entry)].into_iter().collect() }, lottery)
    }

    fn told(held: &[Continuation]) -> Vec<(f64, u32, u32)> {
        held.iter().map(|found| (found.chance, found.map_id, found.stage_id)).collect()
    }

    // A stage's own odds are one roll, and the stage is then drawn evenly from its range.
    #[test]
    fn a_stage_spreads_its_own_odds_over_its_range() {
        let (groups, lottery) = (ExGroup::default(), ExLottery::default());

        assert_eq!(told(&continuations(&ground(10, 81, 0, 0), Some(2271), 0, &groups, &lottery)), [(10.0, 81, 0)]);
        assert_eq!(told(&continuations(&ground(30, 5, 1, 3), None, 0, &groups, &lottery)), [(10.0, 5, 1), (10.0, 5, 2), (10.0, 5, 3)]);
        assert_eq!(told(&continuations(&ground(250, 5, 4, 2), None, 0, &groups, &lottery)), [(100.0, 5, 4)], "odds past certain are certain, and a backwards range is its first stage");
        assert!(continuations(&ground(0, 5, 0, 0), None, 0, &groups, &lottery).is_empty());
        assert!(continuations(&ground(10, 81, 0, 0), None, 0, &groups, &lottery).iter().all(|found| found.category == Category::ExtraStages));
    }

    // A shared table's odds are marks on one roll of a hundred: each row takes what lies between its mark and the one before.
    #[test]
    fn a_table_row_takes_the_odds_between_its_mark_and_the_last() {
        let (groups, lottery) = tables(&[(1, 1), (0, 34), (2, 20), (9, 60)]);
        let plain = continuations(&ground(0, 0, 0, 0), Some(24020), 10, &groups, &lottery);

        assert_eq!(told(&plain), [(1.0, 43, 1), (33.0, 43, 0)], "a mark below the last one can never be rolled, and a row the table lacks leads nowhere");
        assert!(continuations(&ground(0, 0, 0, 0), Some(24020), 9, &groups, &lottery).is_empty());
        assert!(continuations(&ground(0, 0, 0, 0), None, 10, &groups, &lottery).is_empty());
    }

    // The stage's own roll is checked first, so the table only gets what that roll leaves over.
    #[test]
    fn a_table_only_gets_what_the_stages_own_roll_leaves() {
        let (groups, lottery) = tables(&[(0, 50)]);
        let both = continuations(&ground(20, 43, 0, 0), Some(24020), 10, &groups, &lottery);
        let apart = continuations(&ground(20, 44, 2, 2), Some(24020), 10, &groups, &lottery);

        assert_eq!(told(&both), [(60.0, 43, 0)], "the same stage reached both ways is one row");
        assert_eq!(told(&apart), [(20.0, 44, 2), (40.0, 43, 0)]);
    }
}
