use super::arith::BoolWriter;
use super::cost::bit_cost;
use super::tables::{BANDS, DEFAULT_PROBS, EXTRA_PROBS, UPDATE_PROBS};

const FIXED: u16 = 1 << 14;
const SET: u16 = 1 << 15;
const SLOTS: usize = 4 * 8 * 3 * 11;

pub(super) struct Tokens {
    list: Vec<u16>,
    counts: Vec<[u32; 2]>,
}

#[inline]
fn slot(kind: usize, place: usize, context: usize) -> usize {
    ((kind * 8 + BANDS[place]) * 3 + context) * 11
}

impl Tokens {
    pub(super) fn new(capacity: usize) -> Self {
        Self { list: Vec::with_capacity(capacity), counts: vec![[0; 2]; SLOTS] }
    }

    #[inline]
    fn branch(&mut self, value: bool, slot: usize) -> bool {
        self.list.push(slot as u16 | if value { SET } else { 0 });
        self.counts[slot][usize::from(value)] += 1;
        value
    }

    #[inline]
    fn fixed(&mut self, value: bool, prob: u8) {
        self.list.push(u16::from(prob) | FIXED | if value { SET } else { 0 });
    }

    fn extra(&mut self, row: usize, count: usize, value: u32) {
        for (place, &prob) in EXTRA_PROBS[row][..count].iter().enumerate() {
            self.fixed(value >> (count - 1 - place) & 1 == 1, prob);
        }
    }

    pub(super) fn block(&mut self, levels: &[i16; 16], first: usize, kind: usize, context: usize) -> bool {
        let last = levels.iter().rposition(|&level| level != 0).filter(|&last| last >= first);
        let mut base = slot(kind, first, context);
        let Some(last) = last else {
            self.branch(false, base);
            return false;
        };
        self.branch(true, base);
        let mut place = first;
        while place < 16 {
            let level = levels[place];
            place += 1;
            let size = u32::from(level.unsigned_abs());
            if !self.branch(size != 0, base + 1) {
                base = slot(kind, place, 0);
                continue;
            }
            if !self.branch(size > 1, base + 2) {
                base = slot(kind, place, 1);
            } else {
                if !self.branch(size > 4, base + 3) {
                    if self.branch(size != 2, base + 4) {
                        self.branch(size == 4, base + 5);
                    }
                } else if !self.branch(size > 10, base + 6) {
                    if !self.branch(size > 6, base + 7) {
                        self.fixed(size == 6, 159);
                    } else {
                        self.fixed(size >= 9, 165);
                        self.fixed(size & 1 == 0, 145);
                    }
                } else if size < 19 {
                    self.branch(false, base + 8);
                    self.branch(false, base + 9);
                    self.extra(0, 3, size - 11);
                } else if size < 35 {
                    self.branch(false, base + 8);
                    self.branch(true, base + 9);
                    self.extra(1, 4, size - 19);
                } else if size < 67 {
                    self.branch(true, base + 8);
                    self.branch(false, base + 10);
                    self.extra(2, 5, size - 35);
                } else {
                    self.branch(true, base + 8);
                    self.branch(true, base + 10);
                    self.extra(3, 11, size - 67);
                }
                base = slot(kind, place, 2);
            }
            self.fixed(level < 0, 128);
            if place == 16 || !self.branch(place <= last, base) {
                return true;
            }
        }
        true
    }

    pub(super) fn settle(&self, header: &mut BoolWriter) -> Vec<u8> {
        let mut probs = vec![0u8; SLOTS];
        for (index, (prob, count)) in probs.iter_mut().zip(&self.counts).enumerate() {
            let (kind, band, context, node) = (index / 264, index / 33 % 8, index / 11 % 3, index % 11);
            let before = DEFAULT_PROBS[kind][band][context][node];
            let update = UPDATE_PROBS[kind][band][context][node];
            let total = u64::from(count[0]) + u64::from(count[1]);
            let fitted = (u64::from(count[0]) * 255 + total / 2).checked_div(total).map_or(before, |share| share.clamp(1, 255) as u8);
            let spend = |candidate: u8| u64::from(count[0]) * u64::from(bit_cost(false, candidate)) + u64::from(count[1]) * u64::from(bit_cost(true, candidate));
            let keep = spend(before) + u64::from(bit_cost(false, update));
            let swap = spend(fitted) + u64::from(bit_cost(true, update)) + 8 * 256;
            let changed = fitted != before && swap < keep;
            header.bit(changed, update);
            if changed {
                header.literal(u32::from(fitted), 8);
            }
            *prob = if changed { fitted } else { before };
        }
        probs
    }

    pub(super) fn write(&self, probs: &[u8]) -> Vec<u8> {
        let mut writer = BoolWriter::new(self.list.len() / 4 + 16);
        for &token in &self.list {
            let prob = if token & FIXED != 0 { token as u8 } else { probs[usize::from(token & (FIXED - 1))] };
            writer.bit(token & SET != 0, prob);
        }
        writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::cost::bit_cost;
    use super::super::rate::Rates;
    use super::super::tables::DEFAULT_PROBS;
    use super::{FIXED, SET, Tokens};

    #[test]
    fn rate_model_matches_the_coder() {
        // The RD search prices blocks with Rates; it has to agree with what the token coder really emits.
        let rates = Rates::new(&DEFAULT_PROBS);
        let flat: Vec<u8> = DEFAULT_PROBS.iter().flatten().flatten().flatten().copied().collect();
        let mut state = 7u32;
        for round in 0..400 {
            let mut levels = [0i16; 16];
            for level in &mut levels {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let roll = state >> 16;
                *level = match roll % 7 { 0..=3 => 0, 4 => 1, 5 => -(((roll >> 3) % 9) as i16), _ => ((roll >> 3) % 3000) as i16 - 1500 };
            }
            let (first, kind, context) = (round % 2, round % 4, round % 3);
            let mut tokens = Tokens::new(64);
            tokens.block(&levels, first, kind, context);
            let spent: u32 = tokens.list.iter().map(|&token| {
                let prob = if token & FIXED != 0 { token as u8 } else { flat[usize::from(token & (FIXED - 1))] };
                u32::from(bit_cost(token & SET != 0, prob))
            }).sum();
            assert_eq!(spent, rates.block(&levels, first, kind, context), "round {round}");
        }
    }
}
