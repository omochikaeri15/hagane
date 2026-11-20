use std::ops::Range;

use super::{
    Choice, Edges, PLAIN, Tile,
    lossy::lean,
    predict::{DC, HORIZONTAL, LEVEL, PAETH, SMOOTH, SMOOTH_ACROSS, SMOOTH_DOWN, VERTICAL, nearest, predict},
    transform::forward_exact,
};

const STEADY: [u8; 7] = [DC, VERTICAL, HORIZONTAL, PAETH, SMOOTH, SMOOTH_DOWN, SMOOTH_ACROSS];
const MOTION_COST: u32 = 8;

impl Tile<'_> {
    fn uniform(&self, x4: usize, y4: usize, level: u32) -> bool {
        let size = 1usize << level;
        let stride = self.frame.stride;
        let at = y4 * 4 * stride + x4 * 4;
        let (above, left) = (y4 > self.area.top4, x4 > self.area.left4);
        self.frame.planes.iter().all(|plane| {
            let value = plane[at];
            (above || left || value == 128)
                && (0..size).all(|row| plane[at + row * stride..at + row * stride + size].iter().all(|&pixel| pixel == value))
                && (!above || plane[at - stride..at - stride + size].iter().all(|&pixel| pixel == value))
                && (!left || (0..size).all(|row| plane[at + row * stride - 1] == value))
        })
    }

    fn pieces(&self, plane: usize, x4: usize, y4: usize, level: u32, choice: Choice, mut each: impl FnMut(usize, [i32; 16])) {
        let span = 1usize << (level - 2);
        let stride = self.frame.stride;
        let source = &self.frame.planes[plane];
        let mut trial = [0u8; 16];
        for (index, (dx, dy)) in (0..span).flat_map(|dy| (0..span).map(move |dx| (dx, dy))).enumerate() {
            let reach = self.within(x4, y4, dx, dy, 1, span);
            let (x, y) = ((x4 + dx) * 4, (y4 + dy) * 4);
            let edges = Edges::gather(source, stride, x, y, 4, &reach);
            predict(choice.mode, choice.turn, &edges, 4, &mut trial);
            let mut residual = [0i32; 16];
            for (row, line) in residual.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let real = &source[(y + row) * stride + x..(y + row) * stride + x + 4];
                for ((value, &a), &b) in line.iter_mut().zip(real).zip(&trial[row * 4..]) {
                    *value = i32::from(a) - i32::from(b);
                }
            }
            each(index, residual);
        }
    }

    fn weight(&self, planes: Range<usize>, x4: usize, y4: usize, level: u32, choice: Choice) -> u32 {
        let mut total = 0u32;
        for plane in planes {
            self.pieces(plane, x4, y4, level, choice, |_, residual| {
                total += forward_exact(&residual).iter().map(|value| value.unsigned_abs().min(255)).sum::<u32>();
            });
        }
        total
    }

    fn shifted(&self, plane: usize, x4: usize, y4: usize, level: u32, mut each: impl FnMut(usize, [i32; 16])) {
        let span = 1usize << (level - 2);
        let size = span * 4;
        let stride = self.frame.stride;
        let source = &self.frame.planes[plane];
        let guess = &self.guesses.moved[plane];
        for (index, (dx, dy)) in (0..span).flat_map(|dy| (0..span).map(move |dx| (dx, dy))).enumerate() {
            let (x, y) = ((x4 + dx) * 4, (y4 + dy) * 4);
            let mut residual = [0i32; 16];
            for (row, line) in residual.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let real = &source[(y + row) * stride + x..(y + row) * stride + x + 4];
                let near = &guess[(dy * 4 + row) * size + dx * 4..];
                for ((value, &a), &b) in line.iter_mut().zip(real).zip(near) {
                    *value = i32::from(a) - i32::from(b);
                }
            }
            each(index, residual);
        }
    }

    fn cheapest(&self, planes: Range<usize>, x4: usize, y4: usize, level: u32, list: &[Choice]) -> Choice {
        list.iter().copied().min_by_key(|&choice| self.weight(planes.clone(), x4, y4, level, choice)).unwrap_or(PLAIN)
    }

    pub(super) fn exact(&mut self, x4: usize, y4: usize, level: u32) -> bool {
        self.draft.motion = None;
        let mut moved = None;
        if let Some(past) = self.past {
            if self.survey.still(x4, y4, level) {
                self.copy(past, x4, y4, level);
                return true;
            }
            if level < 6 {
                let stack = self.stack(x4, y4, level - 2);
                let mut motion = self.search(past, x4, y4, level, &stack);
                self.guess(past, x4, y4, level, motion);
                if level <= 3 && self.gauge(x4, y4, level).0.iter().any(|&cost| cost != 0) {
                    motion = self.refine(past, x4, y4, level, &stack, motion);
                    self.guess(past, x4, y4, level, motion);
                }
                if self.gauge(x4, y4, level).0.iter().all(|&cost| cost == 0) {
                    self.draft.skip = true;
                    self.draft.tone = PLAIN;
                    self.draft.shade = PLAIN;
                    self.draft.motion = Some(motion);
                    return true;
                }
                moved = Some(motion);
            }
        }
        if self.uniform(x4, y4, level) {
            self.draft.skip = true;
            self.draft.tone = PLAIN;
            self.draft.shade = PLAIN;
            return true;
        }
        if level > 3 {
            return false;
        }
        let stride = self.frame.stride;
        let planes = self.frame.planes.len();
        let mut list = [PLAIN; 10];
        let mut count = 0;
        for mode in STEADY {
            list[count] = Choice { mode, turn: LEVEL };
            count += 1;
        }
        if let Some(angle) = lean(&self.frame.planes[0], stride, y4 * 4 * stride + x4 * 4, 1 << level) {
            let (mode, turn) = nearest(angle, level >= 3);
            let choice = Choice { mode, turn };
            if !list[..count].contains(&choice) {
                list[count] = choice;
                count += 1;
            }
        }
        let tone = self.cheapest(0..1, x4, y4, level, &list[..count]);
        let shade = if planes > 1 {
            let short = [tone, PLAIN, Choice { mode: PAETH, turn: LEVEL }, Choice { mode: VERTICAL, turn: LEVEL }, Choice { mode: HORIZONTAL, turn: LEVEL }];
            self.cheapest(1..3, x4, y4, level, &short)
        } else {
            PLAIN
        };
        let motion = moved.filter(|_| {
            let mut shifted = 0u32;
            for plane in 0..planes {
                self.shifted(plane, x4, y4, level, |_, residual| {
                    shifted += forward_exact(&residual).iter().map(|value| value.unsigned_abs().min(255)).sum::<u32>();
                });
            }
            shifted + MOTION_COST < self.weight(0..1, x4, y4, level, tone) + self.weight(1..planes, x4, y4, level, shade)
        });
        let mut any = 0u32;
        for plane in 0..planes {
            let choice = if plane > 0 { shade } else { tone };
            let mut levels = [0i32; 64];
            let mut store = |index: usize, residual: [i32; 16]| {
                let coefs = forward_exact(&residual);
                any |= coefs.iter().fold(0, |seen, value| seen | value.unsigned_abs());
                levels[index * 16..index * 16 + 16].copy_from_slice(&coefs);
            };
            if motion.is_some() {
                self.shifted(plane, x4, y4, level, &mut store);
            } else {
                self.pieces(plane, x4, y4, level, choice, &mut store);
            }
            self.draft.levels[plane][..64].copy_from_slice(&levels);
        }
        self.draft.skip = any == 0;
        self.draft.tone = tone;
        self.draft.shade = shade;
        self.draft.motion = motion;
        true
    }
}
