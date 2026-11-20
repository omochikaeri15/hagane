use super::cost::bit_cost;
use super::tables::{BANDS, EXTRA_PROBS};

const CAPPED: usize = 68;
const SIGN: u32 = 256;

pub(super) type Probs = [[[[u8; 11]; 3]; 8]; 4];

pub(super) struct Rates {
    probs: Probs,
    levels: Vec<[u16; CAPPED]>,
}

fn magnitude(size: u32, probs: &[u8; 11]) -> u32 {
    let cost = |value: bool, prob: u8| u32::from(bit_cost(value, prob));
    if size == 1 {
        return SIGN + cost(false, probs[2]);
    }
    let mut total = SIGN + cost(true, probs[2]);
    if size <= 4 {
        total += cost(false, probs[3]) + cost(size != 2, probs[4]);
        if size != 2 {
            total += cost(size == 4, probs[5]);
        }
        return total;
    }
    total += cost(true, probs[3]);
    if size <= 10 {
        total += cost(false, probs[6]) + cost(size > 6, probs[7]);
        total += if size <= 6 { cost(size == 6, 159) } else { cost(size >= 9, 165) + cost(size & 1 == 0, 145) };
        return total;
    }
    total += cost(true, probs[6]);
    let (row, count, base, upper, lower) = match size {
        11..=18 => (0, 3, 11, false, false),
        19..=34 => (1, 4, 19, false, true),
        35..=66 => (2, 5, 35, true, false),
        _ => (3, 11, 67, true, true),
    };
    total += cost(upper, probs[8]) + cost(lower, probs[if upper { 10 } else { 9 }]);
    let extra = size - base;
    for (place, &prob) in EXTRA_PROBS[row][..count].iter().enumerate() {
        total += cost(extra >> (count - 1 - place) & 1 == 1, prob);
    }
    total
}

impl Rates {
    pub(super) fn new(probs: &Probs) -> Self {
        let mut levels = vec![[0u16; CAPPED]; 4 * 8 * 3];
        for (slot, table) in levels.iter_mut().enumerate() {
            let node = &probs[slot / 24][slot / 3 % 8][slot % 3];
            for (size, cost) in table.iter_mut().enumerate().skip(1) {
                *cost = magnitude(size as u32, node).min(u32::from(u16::MAX)) as u16;
            }
        }
        Self { probs: *probs, levels }
    }

    #[inline]
    fn level(&self, kind: usize, band: usize, context: usize, size: u32) -> u32 {
        let index = (kind * 8 + band) * 3 + context;
        if (size as usize) < CAPPED { u32::from(self.levels[index][size as usize]) } else { magnitude(size, &self.probs[kind][band][context]) }
    }

    #[inline]
    pub(super) fn more(&self, kind: usize, place: usize, context: usize) -> u32 {
        u32::from(bit_cost(true, self.probs[kind][BANDS[place]][context][0]))
    }

    #[inline]
    pub(super) fn stop(&self, kind: usize, place: usize, context: usize) -> u32 {
        u32::from(bit_cost(false, self.probs[kind][BANDS[place]][context][0]))
    }

    #[inline]
    pub(super) fn token(&self, kind: usize, place: usize, context: usize, size: u32) -> u32 {
        let band = BANDS[place];
        let node = u32::from(bit_cost(size != 0, self.probs[kind][band][context][1]));
        if size == 0 { node } else { node + self.level(kind, band, context, size) }
    }

    pub(super) fn block(&self, levels: &[i16; 16], first: usize, kind: usize, context: usize) -> u32 {
        let probs = &self.probs[kind];
        let Some(last) = levels.iter().rposition(|&level| level != 0).filter(|&last| last >= first) else {
            return u32::from(bit_cost(false, probs[BANDS[first]][context][0]));
        };
        let mut total = u32::from(bit_cost(true, probs[BANDS[first]][context][0]));
        let mut context = context;
        let mut place = first;
        while place <= last {
            let band = BANDS[place];
            let size = u32::from(levels[place].unsigned_abs());
            place += 1;
            if size == 0 {
                total += u32::from(bit_cost(false, probs[band][context][1]));
                context = 0;
                continue;
            }
            total += u32::from(bit_cost(true, probs[band][context][1])) + self.level(kind, band, context, size);
            context = if size == 1 { 1 } else { 2 };
            if place < 16 {
                total += u32::from(bit_cost(place <= last, probs[BANDS[place]][context][0]));
            }
        }
        total
    }
}
