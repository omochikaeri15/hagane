use super::tables::{BIT_COSTS, BLOCK_MODE_PROBS, CHROMA_MODE_PROBS, LUMA_MODE_PROBS};

pub(super) const SMALL_PATHS: [&[(u8, bool)]; 10] = [
    &[(0, false)],
    &[(0, true), (1, false)],
    &[(0, true), (1, true), (2, false)],
    &[(0, true), (1, true), (2, true), (3, false), (4, false)],
    &[(0, true), (1, true), (2, true), (3, true), (6, false)],
    &[(0, true), (1, true), (2, true), (3, false), (4, true), (5, false)],
    &[(0, true), (1, true), (2, true), (3, false), (4, true), (5, true)],
    &[(0, true), (1, true), (2, true), (3, true), (6, true), (7, false)],
    &[(0, true), (1, true), (2, true), (3, true), (6, true), (7, true), (8, false)],
    &[(0, true), (1, true), (2, true), (3, true), (6, true), (7, true), (8, true)],
];
pub(super) const LARGE_PATHS: [&[(u8, bool)]; 5] = [
    &[(0, true), (1, false), (2, false)],
    &[(0, true), (1, false), (2, true)],
    &[(0, true), (1, true), (3, false)],
    &[(0, true), (1, true), (3, true)],
    &[(0, false)],
];
pub(super) const CHROMA_PATHS: [&[(u8, bool)]; 4] = [&[(0, false)], &[(0, true), (1, false)], &[(0, true), (1, true), (2, false)], &[(0, true), (1, true), (2, true)]];
pub(super) const SMALL_COSTS: [[[u16; 10]; 10]; 10] = small_costs();
pub(super) const LARGE_COSTS: [u16; 5] = [walk(LARGE_PATHS[0], &LUMA_MODE_PROBS), walk(LARGE_PATHS[1], &LUMA_MODE_PROBS), walk(LARGE_PATHS[2], &LUMA_MODE_PROBS), walk(LARGE_PATHS[3], &LUMA_MODE_PROBS), walk(LARGE_PATHS[4], &LUMA_MODE_PROBS)];
pub(super) const CHROMA_COSTS: [u16; 4] = [walk(CHROMA_PATHS[0], &CHROMA_MODE_PROBS), walk(CHROMA_PATHS[1], &CHROMA_MODE_PROBS), walk(CHROMA_PATHS[2], &CHROMA_MODE_PROBS), walk(CHROMA_PATHS[3], &CHROMA_MODE_PROBS)];

#[inline]
pub(super) const fn bit_cost(value: bool, prob: u8) -> u16 {
    BIT_COSTS[if value { 256 - prob as usize } else { prob as usize }]
}

const fn walk(path: &[(u8, bool)], probs: &[u8]) -> u16 {
    let mut cost = 0;
    let mut step = 0;
    while step < path.len() {
        cost += bit_cost(path[step].1, probs[path[step].0 as usize]);
        step += 1;
    }
    cost
}

const fn small_costs() -> [[[u16; 10]; 10]; 10] {
    let mut costs = [[[0u16; 10]; 10]; 10];
    let mut above = 0;
    while above < 10 {
        let mut left = 0;
        while left < 10 {
            let mut mode = 0;
            while mode < 10 {
                costs[above][left][mode] = walk(SMALL_PATHS[mode], &BLOCK_MODE_PROBS[above][left]);
                mode += 1;
            }
            left += 1;
        }
        above += 1;
    }
    costs
}
