use super::{CLEAR, FRESH, INTER, INTRA, PLAIN, Past, Plane, SIZE_GROUP, Source, Tile, VOID, Window, lossy::Pick, motion, stack::Stack};

const INTRA_BITS: u32 = 4;

struct Probe {
    x4: usize,
    y4: usize,
    level: u32,
    list: [(usize, usize, usize); 16],
    count: usize,
}
const FARTHEST: i32 = 1024;
const FRACTIONS: [i32; 2] = [4, 2];
const NEIGHBORS: [(i32, i32); 8] = [(-1, 0), (0, -1), (0, 1), (1, 0), (-1, -1), (-1, 1), (1, -1), (1, 1)];

fn view<'b>(past: &Past<'b>, frame: &Source<'_>, plane: usize) -> Plane<'b> {
    Plane { data: &past.recon[plane], stride: frame.stride, width: frame.width, height: frame.height }
}

fn length(value: i32) -> u32 {
    if value == 0 { 0 } else { 3 + 2 * (value.unsigned_abs() / 8 + 1).ilog2() }
}

fn spread(want: [i32; 2], base: [i32; 2]) -> u32 {
    1 + length(want[0] - base[0]) + length(want[1] - base[1])
}

fn price(stack: &Stack, want: [i32; 2]) -> u32 {
    if want == [0, 0] {
        return 2;
    }
    if stack.found > 0 && want == stack.mvs[0] {
        return 3;
    }
    if (1..4).any(|index| (index == 1 || stack.found > index) && stack.mvs[index] == want) {
        return 4;
    }
    let choices = 1 + usize::from(stack.found > 1) + usize::from(stack.found > 2);
    3 + (0..choices).map(|index| spread(want, stack.mvs[index]) + index as u32).min().unwrap_or(0)
}

impl Tile<'_> {
    fn lambda(&self) -> u32 {
        (self.ac[0].size as u32 / 32).max(1)
    }

    pub(super) fn copy(&mut self, past: &Past<'_>, x4: usize, y4: usize, level: u32) {
        let size = 1usize << level;
        let (x, y) = (x4 * 4, y4 * 4);
        let stride = self.frame.stride;
        for plane in 0..self.frame.planes.len() {
            let block = &mut self.scratch.trial[..size * size];
            motion::fetch(&view(past, self.frame, plane), x as isize, y as isize, size, block);
            for (row, line) in block.chunks_exact(size).enumerate() {
                let at = (y + row) * stride + x;
                self.recon[plane][at..at + size].copy_from_slice(line);
            }
        }
        self.draft.skip = true;
        self.draft.tone = PLAIN;
        self.draft.shade = PLAIN;
        self.draft.motion = Some([0, 0]);
    }

    pub(super) fn guess(&mut self, past: &Past<'_>, x4: usize, y4: usize, level: u32, mv: [i32; 2]) {
        let size = 1usize << level;
        for plane in 0..self.frame.planes.len() {
            motion::predict(&view(past, self.frame, plane), (x4 * 4) as isize, (y4 * 4) as isize, size, mv, &mut self.guesses.moved[plane][..size * size]);
        }
    }

    pub(super) fn gauge(&self, x4: usize, y4: usize, level: u32) -> ([u32; 3], u32) {
        let size = 1usize << level;
        let piece = size.min(8);
        let cells = size.div_ceil(8);
        let stride = self.frame.stride;
        let at = y4 * 4 * stride + x4 * 4;
        let mut costs = [0u32; 3];
        let mut swing = 0u32;
        for (plane, cost) in costs.iter_mut().enumerate().take(self.frame.planes.len()) {
            let source = &self.frame.planes[plane];
            let guess = &self.guesses.moved[plane];
            let (mut low, mut high) = (i32::MAX, i32::MIN);
            for cell in 0..cells * cells {
                let (left, top) = ((cell % cells) * 8, (cell / cells) * 8);
                if level > 3 && self.survey.void(x4 + left / 4, y4 + top / 4, 3) == VOID {
                    continue;
                }
                for row in top..top + piece {
                    let real = &source[at + row * stride + left..at + row * stride + left + piece];
                    for (&a, &b) in real.iter().zip(&guess[row * size + left..row * size + left + piece]) {
                        let delta = i32::from(a) - i32::from(b);
                        *cost += delta.unsigned_abs();
                        low = low.min(delta);
                        high = high.max(delta);
                    }
                }
            }
            if low <= high {
                swing = swing.max((high - low) as u32);
            }
        }
        (costs, swing)
    }

    fn probe(&self, x4: usize, y4: usize, level: u32) -> Probe {
        let size = 1usize << level;
        let cells = size.div_ceil(8);
        let mut list = [(0usize, 0usize, size); 16];
        let mut count = 1;
        if level > 3 && self.survey.void(x4, y4, level) != CLEAR {
            count = 0;
            for cell in (0..cells * cells).filter(|&cell| self.survey.void(x4 + (cell % cells) * 2, y4 + (cell / cells) * 2, 3) != VOID) {
                list[count] = ((cell % cells) * 8, (cell / cells) * 8, 8);
                count += 1;
            }
        }
        Probe { x4, y4, level, list, count }
    }

    fn measure(&self, past: &Past<'_>, stack: &Stack, probe: &Probe, mv: [i32; 2]) -> u32 {
        let size = 1usize << probe.level;
        let (x, y) = ((probe.x4 * 4) as isize, (probe.y4 * 4) as isize);
        let stride = self.frame.stride;
        let at = probe.y4 * 4 * stride + probe.x4 * 4;
        let source = &self.frame.planes[0];
        let plane = view(past, self.frame, 0);
        let (dx, dy) = ((mv[1] >> 3) as isize, (mv[0] >> 3) as isize);
        let cells = &probe.list[..probe.count];
        let sad: u32 = if mv[0] & 7 == 0 && mv[1] & 7 == 0 {
            cells.iter().map(|&(left, top, span)| motion::distance(&plane, x + dx + left as isize, y + dy + top as isize, span, &Window { source, at: at + top * stride + left, stride })).sum()
        } else if let Some(total) = cells.iter().map(|&(left, top, span)| past.phases.distance(x + left as isize, y + top as isize, span, mv, &Window { source, at: at + top * stride + left, stride })).sum::<Option<u32>>() {
            total
        } else {
            let mut block = [0u8; 1024];
            motion::predict(&plane, x, y, size, mv, &mut block[..size * size]);
            cells
                .iter()
                .map(|&(left, top, span)| {
                    (top..top + span)
                        .map(|row| {
                            let real = &source[at + row * stride + left..at + row * stride + left + span];
                            real.iter().zip(&block[row * size + left..row * size + left + span]).map(|(&a, &b)| u32::from(a.abs_diff(b))).sum::<u32>()
                        })
                        .sum::<u32>()
                })
                .sum()
        };
        sad + self.lambda() * price(stack, mv)
    }

    pub(super) fn search(&mut self, past: &Past<'_>, x4: usize, y4: usize, level: u32, stack: &Stack) -> [i32; 2] {
        let probe = self.probe(x4, y4, level);
        let measure = |mv: [i32; 2]| self.measure(past, stack, &probe, mv);
        let mut best = ([0, 0], measure([0, 0]));
        let hint = self.hints[level as usize + 1];
        for candidate in stack.mvs[..stack.found].iter().chain([&hint]) {
            let mv = candidate.map(|value| ((value + 4) >> 3) << 3);
            if mv != best.0 && mv.iter().all(|value| value.abs() <= FARTHEST) {
                let cost = measure(mv);
                if cost < best.1 {
                    best = (mv, cost);
                }
            }
        }
        for _ in 0..self.tuning.reach {
            let center = best.0;
            let mut moved = false;
            for (down, across) in NEIGHBORS {
                let mv = [center[0] + 8 * down, center[1] + 8 * across];
                if mv.iter().any(|value| value.abs() > FARTHEST) {
                    continue;
                }
                let cost = measure(mv);
                if cost < best.1 {
                    best = (mv, cost);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        self.hints[level as usize] = best.0;
        best.0
    }

    pub(super) fn refine(&self, past: &Past<'_>, x4: usize, y4: usize, level: u32, stack: &Stack, start: [i32; 2]) -> [i32; 2] {
        let probe = self.probe(x4, y4, level);
        let measure = |mv: [i32; 2]| self.measure(past, stack, &probe, mv);
        let mut best = (start, measure(start));
        for &mv in &stack.mvs[..stack.found] {
            if mv != best.0 && (mv[0] & 7 != 0 || mv[1] & 7 != 0) && mv.iter().all(|value| value.abs() <= FARTHEST) {
                let cost = measure(mv);
                if cost < best.1 {
                    best = (mv, cost);
                }
            }
        }
        for &step in &FRACTIONS[..self.tuning.fractions] {
            let center = best.0;
            for (down, across) in NEIGHBORS {
                let mv = [center[0] + step * down, center[1] + step * across];
                if mv.iter().all(|value| value.abs() <= FARTHEST) {
                    let cost = measure(mv);
                    if cost < best.1 {
                        best = (mv, cost);
                    }
                }
            }
        }
        best.0
    }

    pub(super) fn steer(&mut self, x4: usize, y4: usize, level: u32, force: bool) -> bool {
        let Some(past) = self.past else {
            return false;
        };
        let void = self.survey.void(x4, y4, level);
        if void == VOID || self.survey.still(x4, y4, level) {
            self.copy(past, x4, y4, level);
            return true;
        }
        if level == 6 {
            return false;
        }
        let stack = self.stack(x4, y4, level - 2);
        let mut motion = self.search(past, x4, y4, level, &stack);
        if self.tuning.eager {
            motion = self.refine(past, x4, y4, level, &stack, motion);
        }
        self.guess(past, x4, y4, level, motion);
        let (mut costs, swing) = self.gauge(x4, y4, level);
        let calm = self.tuning.calm[level as usize - 3];
        let lambda = self.lambda();
        let size = 1usize << level;
        let flat = void == CLEAR && self.survey.swing(x4, y4, level) <= calm;
        if force && swing as f32 > self.tuning.prune[level as usize - 4] * calm.max(1) as f32 {
            return false;
        }
        let whole = level == 3 || swing <= calm || force;
        if !whole && !flat {
            return false;
        }
        if whole && !self.tuning.eager {
            let finer = self.refine(past, x4, y4, level, &stack, motion);
            if finer != motion {
                motion = finer;
                self.guess(past, x4, y4, level, motion);
                costs = self.gauge(x4, y4, level).0;
            }
        }
        let bits = price(&stack, motion);
        let moved = Pick { tone: PLAIN, shade: PLAIN, sides: [0; 3], costs, motion: Some(motion) };
        let limit = self.quiet(size);
        if costs.iter().all(|&cost| cost <= limit) || (whole && !flat && level > 3) {
            self.code(x4, y4, level, &moved);
            return true;
        }
        let intra = self.choose(x4, y4, level);
        let rival = intra.costs.iter().sum::<u32>() + lambda * INTRA_BITS;
        let price = costs.iter().sum::<u32>() + lambda * (bits + 1);
        if whole && price <= rival {
            self.code(x4, y4, level, &moved);
            return true;
        }
        if !flat {
            return false;
        }
        self.code(x4, y4, level, &intra);
        true
    }

    pub(super) fn intent(&mut self, x4: usize, y4: usize, level: u32, has_above: bool, has_left: bool) {
        let above = has_above.then(|| self.cell(x4, y4 - 1).kind >= INTER);
        let left = has_left.then(|| self.cell(x4 - 1, y4).kind >= INTER);
        let context = match (above, left) {
            (Some(up), Some(side)) => {
                if !up && !side {
                    3
                } else {
                    usize::from(!up || !side)
                }
            }
            (Some(up), None) => 2 * usize::from(!up),
            (None, Some(side)) => 2 * usize::from(!side),
            (None, None) => 0,
        };
        let rank = level - 2;
        let (tone, shade) = (self.draft.tone, self.draft.shade);
        if let Some(want) = self.draft.motion {
            self.coder.symbol(1, &mut self.model.is_inter[context]);
            let count = usize::from(above == Some(true)) + usize::from(left == Some(true));
            let refs = 1 + usize::from(count > 0);
            for index in [0, 2, 3] {
                self.coder.symbol(0, &mut self.model.single_ref[refs][index]);
            }
            let stack = self.stack(x4, y4, rank);
            let fresh = self.signal(&stack, want);
            self.paint(x4, y4, rank, want, if fresh { FRESH } else { INTER });
        } else {
            self.coder.symbol(0, &mut self.model.is_inter[context]);
            self.coder.symbol(usize::from(tone.mode), &mut self.model.inter_y_mode[usize::from(SIZE_GROUP[rank as usize])]);
            self.angles(level, tone, shade);
            self.paint(x4, y4, rank, [0, 0], INTRA);
        }
    }

    fn signal(&mut self, stack: &Stack, want: [i32; 2]) -> bool {
        let found = stack.found;
        if want == [0, 0] {
            self.coder.symbol(1, &mut self.model.new_mv[stack.fresh_context]);
            self.coder.symbol(0, &mut self.model.zero_mv[0]);
            return false;
        }
        if found > 0 && want == stack.mvs[0] {
            self.coder.symbol(1, &mut self.model.new_mv[stack.fresh_context]);
            self.coder.symbol(1, &mut self.model.zero_mv[0]);
            self.coder.symbol(0, &mut self.model.ref_mv[stack.near_context]);
            return false;
        }
        if let Some(near) = (1..4).find(|&index| (index == 1 || found > index) && stack.mvs[index] == want) {
            self.coder.symbol(1, &mut self.model.new_mv[stack.fresh_context]);
            self.coder.symbol(1, &mut self.model.zero_mv[0]);
            self.coder.symbol(1, &mut self.model.ref_mv[stack.near_context]);
            for index in 1..3 {
                if found > index + 1 {
                    self.coder.symbol(usize::from(index != near), &mut self.model.drl_mode[stack.drl[index]]);
                    if index == near {
                        break;
                    }
                }
            }
            return false;
        }
        self.coder.symbol(0, &mut self.model.new_mv[stack.fresh_context]);
        let choices = 1 + usize::from(found > 1) + usize::from(found > 2);
        let base = (0..choices).min_by_key(|&index| spread(want, stack.mvs[index]) + index as u32).unwrap_or(0);
        for index in 0..2 {
            if found > index + 1 {
                self.coder.symbol(usize::from(index != base), &mut self.model.drl_mode[stack.drl[index]]);
                if index == base {
                    break;
                }
            }
        }
        let start = stack.mvs[if found <= 1 { 0 } else { base }];
        self.vector([want[0] - start[0], want[1] - start[1]]);
        true
    }

    fn vector(&mut self, delta: [i32; 2]) {
        let joint = usize::from(delta[1] != 0) | (usize::from(delta[0] != 0) << 1);
        self.coder.symbol(joint, &mut self.model.mv_joint);
        for (axis, &value) in delta.iter().enumerate() {
            if value != 0 {
                self.component(axis, value);
            }
        }
    }

    fn component(&mut self, axis: usize, value: i32) {
        self.coder.symbol(usize::from(value < 0), &mut self.model.mv_sign[axis]);
        let offset = value.unsigned_abs() - 1;
        if offset < 16 {
            self.coder.symbol(0, &mut self.model.mv_class[axis]);
            let high = (offset >> 3) as usize;
            self.coder.symbol(high, &mut self.model.mv_class0_bit[axis]);
            self.coder.symbol(((offset >> 1) & 3) as usize, &mut self.model.mv_class0_fr[axis][high]);
        } else {
            let class = (offset >> 3).ilog2();
            self.coder.symbol(class as usize, &mut self.model.mv_class[axis]);
            let rest = offset - (8 << class);
            for bit in 0..class {
                self.coder.symbol(((rest >> (3 + bit)) & 1) as usize, &mut self.model.mv_bit[axis][bit as usize]);
            }
            self.coder.symbol(((rest >> 1) & 3) as usize, &mut self.model.mv_fr[axis]);
        }
    }
}
