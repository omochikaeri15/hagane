use super::{
    IDENTITY, Tile,
    tables::{BASE_OFFSET, DEFAULT_SCAN_4X4, DEFAULT_SCAN_8X8, DEFAULT_SCAN_16X16, DEFAULT_SCAN_32X32},
};

const WIDE_SYMBOL: [usize; 5] = [1, 5, 6, 4, 0];
const NARROW_SYMBOL: [usize; 5] = [1, 3, 4, 2, 0];
const ONE_SYMBOL: [usize; 10] = [7, 8, 9, 12, 0, 10, 11, 13, 14, 15];
const TWO_SYMBOL: [usize; 10] = [3, 4, 5, 8, 0, 6, 7, 9, 10, 11];

pub(super) fn scan(size: usize) -> &'static [u16] {
    match size {
        4 => &DEFAULT_SCAN_4X4,
        8 => &DEFAULT_SCAN_8X8,
        16 => &DEFAULT_SCAN_16X16,
        _ => &DEFAULT_SCAN_32X32,
    }
}

impl Tile<'_> {
    pub(super) fn coefficients(&mut self, plane: usize, x4: usize, y4: usize, size: usize, whole: bool, from: usize) {
        let kind = self.draft.kinds[plane];
        let mode = self.draft.tone.mode;
        let levels = &self.draft.levels[plane][from..];
        let area = size * size;
        let span = size >> 2;
        let row4 = y4 & 15;
        let shift = size.trailing_zeros();
        let class = shift as usize - 2;
        let kin = usize::from(plane > 0);
        let order = scan(size);
        let eob = order.iter().rposition(|&at| levels[usize::from(at)] != 0).map_or(0, |last| last + 1);
        let above_level = &mut self.above.level[plane][x4..x4 + span];
        let above_sign = &mut self.above.sign[plane][x4..x4 + span];
        let left_level = &mut self.left.level[plane][row4..row4 + span];
        let left_sign = &mut self.left.sign[plane][row4..row4 + span];
        let context = if plane > 0 {
            let above = above_level.iter().chain(above_sign.iter()).any(|&value| value != 0);
            let left = left_level.iter().chain(left_sign.iter()).any(|&value| value != 0);
            7 + usize::from(above) + usize::from(left) + if whole { 0 } else { 3 }
        } else if whole {
            0
        } else {
            let top = above_level.iter().copied().max().unwrap_or(0);
            let side = left_level.iter().copied().max().unwrap_or(0);
            match (top.min(side), top.max(side)) {
                (0, 0) => 1,
                (0, most) => 2 + usize::from(most > 3),
                (_, most) if most <= 3 => 4,
                (least, _) if least <= 3 => 5,
                _ => 6,
            }
        };
        self.coder.symbol(usize::from(eob == 0), &mut self.model.all_zero[class][context]);
        if eob == 0 {
            above_level.fill(0);
            above_sign.fill(0);
            left_level.fill(0);
            left_sign.fill(0);
            return;
        }
        if plane == 0 && self.tuning.base_q > 0 {
            let reduced = self.past.is_some() && !self.tuning.bend;
            if self.draft.motion.is_some() {
                match size {
                    _ if reduced || size == 32 => self.coder.symbol(usize::from(kind != IDENTITY), &mut self.model.inter_tx_type[class]),
                    16 => self.coder.symbol(TWO_SYMBOL[usize::from(kind)], &mut self.model.inter_tx_two),
                    _ => self.coder.symbol(ONE_SYMBOL[usize::from(kind)], &mut self.model.inter_tx_one[class]),
                }
            } else if size == 16 || (size < 16 && reduced) {
                self.coder.symbol(NARROW_SYMBOL[usize::from(kind)], &mut self.model.tx_type_narrow[class][usize::from(mode)]);
            } else if size < 16 {
                self.coder.symbol(WIDE_SYMBOL[usize::from(kind)], &mut self.model.tx_type_wide[class][usize::from(mode)]);
            }
        }
        let group = (32 - (eob as u32 - 1).leading_zeros()) as usize;
        match size {
            4 => self.coder.symbol(group, &mut self.model.eob_16[kin][0]),
            8 => self.coder.symbol(group, &mut self.model.eob_64[kin][0]),
            16 => self.coder.symbol(group, &mut self.model.eob_256[kin][0]),
            _ => self.coder.symbol(group, &mut self.model.eob_1024[kin]),
        }
        if group >= 2 {
            let offset = eob - ((1usize << (group - 1)) + 1);
            let top = group - 2;
            self.coder.symbol((offset >> top) & 1, &mut self.model.eob_extra[class][kin][group - 2]);
            for bit in (0..top).rev() {
                self.coder.bit((offset >> bit) & 1 != 0);
            }
        }
        let pitch = size + 2;
        let sizes = &mut self.scratch.sizes;
        for &at in &order[..eob] {
            let at = usize::from(at);
            sizes[(at >> shift) * pitch + (at & (size - 1))] = levels[at].unsigned_abs().min(15) as u8;
        }
        for index in (0..eob).rev() {
            let at = usize::from(order[index]);
            let (row, column) = (at >> shift, at & (size - 1));
            let here = row * pitch + column;
            let level = levels[at].unsigned_abs();
            if index == eob - 1 {
                let context = if index == 0 {
                    0
                } else if index <= area / 8 {
                    1
                } else if index <= area / 4 {
                    2
                } else {
                    3
                };
                self.coder.symbol(level.min(3) as usize - 1, &mut self.model.base_eob[class][kin][context]);
            } else {
                let context = if at == 0 {
                    0
                } else {
                    let near = [here + 1, here + pitch, here + pitch + 1, here + 2, here + 2 * pitch];
                    let sum: u32 = near.iter().map(|&spot| u32::from(sizes[spot].min(3))).sum();
                    ((sum + 1) >> 1).min(4) as usize + usize::from(BASE_OFFSET[class][row.min(4)][column.min(4)])
                };
                self.coder.symbol(level.min(3) as usize, &mut self.model.base[class][kin][context]);
            }
            if level > 2 {
                let sum = u32::from(sizes[here + 1]) + u32::from(sizes[here + pitch]) + u32::from(sizes[here + pitch + 1]);
                let near = ((sum + 1) >> 1).min(6) as usize;
                let context = if at == 0 {
                    near
                } else if row < 2 && column < 2 {
                    near + 7
                } else {
                    near + 14
                };
                let mut rest = level - 3;
                for _ in 0..4 {
                    let part = rest.min(3);
                    self.coder.symbol(part as usize, &mut self.model.range[class][kin][context]);
                    rest -= part;
                    if part < 3 {
                        break;
                    }
                }
            }
        }
        let mut total = 0u32;
        for (index, &at) in order[..eob].iter().enumerate() {
            let level = levels[usize::from(at)];
            if level == 0 {
                continue;
            }
            let size = level.unsigned_abs();
            total += size;
            if index == 0 {
                let mut lean = 0i32;
                for &mark in above_sign.iter().chain(left_sign.iter()) {
                    lean += i32::from(mark == 2) - i32::from(mark == 1);
                }
                let context = match lean {
                    i32::MIN..=-1 => 1,
                    0 => 0,
                    _ => 2,
                };
                self.coder.symbol(usize::from(level < 0), &mut self.model.dc_sign[kin][context]);
            } else {
                self.coder.bit(level < 0);
            }
            if size > 14 {
                let rest = size - 14;
                let length = 32 - rest.leading_zeros();
                for _ in 1..length {
                    self.coder.bit(false);
                }
                for bit in (0..length).rev() {
                    self.coder.bit((rest >> bit) & 1 != 0);
                }
            }
        }
        for &at in &order[..eob] {
            let at = usize::from(at);
            sizes[(at >> shift) * pitch + (at & (size - 1))] = 0;
        }
        let weight = total.min(63) as u8;
        let mark = match levels[0] {
            0 => 0,
            i32::MIN..=-1 => 1,
            _ => 2,
        };
        above_level.fill(weight);
        above_sign.fill(mark);
        left_level.fill(weight);
        left_sign.fill(mark);
    }
}
