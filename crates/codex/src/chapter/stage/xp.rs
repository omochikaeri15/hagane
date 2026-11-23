use std::ops::RangeInclusive;

const STORY_MAPS: RangeInclusive<u32> = 3000..=3008;
const STORY_ROWS: usize = 51;
const ROW_STEP: u32 = 6000;
const FLOOR: u32 = 20000;
const SCALE: u32 = 10;
const CLEARS_COUNTED: u32 = 13;
const CLEAR_SHARE: u32 = 14;

pub fn story_xp(global_map_id: u32, stage: usize) -> u32 {
    if !STORY_MAPS.contains(&global_map_id) || stage >= STORY_ROWS {
        return 0;
    }

    (stage as u32 * ROW_STEP + FLOOR) / SCALE
}

pub fn story_xp_after(base: u32, clears: u32) -> u32 {
    base - clears.min(CLEARS_COUNTED) * base / CLEAR_SHARE
}

#[cfg(test)]
mod tests {
    use super::*;

    // A story stage pays by its place in the chapter and nothing else: 2000 for the first, 600 more for each one after.
    #[test]
    fn a_story_stage_pays_by_its_place() {
        assert_eq!((story_xp(3000, 0), story_xp(3000, 1), story_xp(3000, 47)), (2000, 2600, 30200));
        assert_eq!(story_xp(3004, 12), story_xp(3008, 12), "every chapter of every saga pays the same");
        assert_eq!((story_xp(3001, 49), story_xp(3002, 50)), (31400, 32000), "the second and third moons sit past the forty-eighth place");
        assert_eq!((story_xp(3000, 51), story_xp(2999, 0), story_xp(3009, 0)), (0, 0, 0));
    }

    // Each earlier clear takes a fourteenth of the first payout away, and it stops falling after thirteen.
    #[test]
    fn a_repeat_clear_pays_less() {
        assert_eq!((story_xp_after(2000, 0), story_xp_after(2000, 1), story_xp_after(2000, 7)), (2000, 1858, 1000));
        assert_eq!((story_xp_after(2000, 13), story_xp_after(2000, 99)), (143, 143));
    }
}
