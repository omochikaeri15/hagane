use std::ops::Range;

use super::{
    COSINE, Choice, Edges, FLIP, FLIP_ACROSS, FLIP_DOWN, FLIP_SINE, IDENTITY, PLAIN, Reach, SINE, SINE_ACROSS, SINE_DOWN, SINE_FLIP, Source, Step, Tile, VOID,
    coefs::scan,
    predict::{DC, HORIZONTAL, LEVEL, PAETH, SMOOTH, SMOOTH_ACROSS, SMOOTH_DOWN, VERTICAL, nearest, predict},
    tables::MODE_TRANSFORM,
    transform,
};

const STEADY: [u8; 7] = [DC, VERTICAL, HORIZONTAL, PAETH, SMOOTH, SMOOTH_DOWN, SMOOTH_ACROSS];
const TINTS: [u8; 3] = [DC, SMOOTH, PAETH];
const BEND_SHARE: i64 = 25;
const BENDS: [u8; 8] = [SINE, SINE_DOWN, SINE_ACROSS, FLIP, FLIP_DOWN, FLIP_ACROSS, SINE_FLIP, FLIP_SINE];

fn distance(trial: &[u8], source: &[u8], stride: usize, at: usize, size: usize) -> u32 {
    trial
        .chunks_exact(size)
        .take(size)
        .zip(source[at..].chunks(stride))
        .map(|(line, real)| line.iter().zip(real).map(|(&a, &b)| u32::from(a.abs_diff(b))).sum::<u32>())
        .sum()
}

pub(super) fn lean(source: &[u8], stride: usize, at: usize, size: usize) -> Option<i32> {
    let (mut across, mut down, mut cross) = (0i32, 0i32, 0i32);
    for row in 0..size - 1 {
        let line = &source[at + row * stride..at + row * stride + size];
        let next = &source[at + (row + 1) * stride..at + (row + 1) * stride + size];
        for ((pair, &below), _) in line.windows(2).zip(next).zip(0..size - 1) {
            let dx = i32::from(pair[1]) - i32::from(pair[0]);
            let dy = i32::from(below) - i32::from(pair[0]);
            across += dx * dx;
            down += dy * dy;
            cross += dx * dy;
        }
    }
    let energy = across + down;
    if energy < (size * size) as i32 {
        return None;
    }
    let (spread, twist) = ((across - down) as f32, (2 * cross) as f32);
    if spread * spread + twist * twist < 0.16 * (energy as f32) * (energy as f32) {
        return None;
    }
    let slope = 0.5 * twist.atan2(spread).to_degrees();
    Some((90.0 - slope).rem_euclid(180.0).round() as i32 % 180)
}

fn difference(out: &mut [i32], source: &[u8], guess: &[u8], stride: usize, size: usize, hollow: &[bool]) {
    let cells = size.div_ceil(8);
    for (row, (line, guess)) in out.chunks_exact_mut(size).zip(guess.chunks_exact(size)).enumerate() {
        let real = &source[row * stride..row * stride + size];
        let hollow = &hollow[(row / 8) * cells..];
        for (column, ((value, &a), &b)) in line.iter_mut().zip(real).zip(guess).enumerate() {
            *value = if hollow[column / 8] { 0 } else { i32::from(a) - i32::from(b) };
        }
    }
}

fn quantize(coefs: &[i32], levels: &mut [i32], dc: &Step, ac: &Step, bias: u32, double: u32) -> bool {
    let mut any = 0;
    let round = (ac.size as u32 * bias) >> 6;
    let floor = (ac.size as u32 - round).div_ceil(1 << double);
    for (level, &coef) in levels[1..].iter_mut().zip(&coefs[1..]) {
        let size = coef.unsigned_abs();
        *level = if size < floor {
            0
        } else {
            let magnitude = (((size << double) + round) as f32 * ac.flip) as i32;
            any |= magnitude;
            if coef < 0 { -magnitude } else { magnitude }
        };
    }
    let first = coefs[0];
    let magnitude = (((first.unsigned_abs() << double) + ((dc.size as u32 * bias) >> 6)) as f32 * dc.flip) as i32;
    levels[0] = if first < 0 { -magnitude } else { magnitude };
    (any | magnitude) != 0
}

fn restore(levels: &[i32], coefs: &mut [i32], dc: &Step, ac: &Step, double: u32) {
    for (coef, &level) in coefs.iter_mut().zip(levels) {
        let magnitude = ((level.abs() * ac.size) & 0xFF_FFFF) >> double;
        *coef = (if level < 0 { -magnitude } else { magnitude }).clamp(-32768, 32767);
    }
    let magnitude = ((levels[0].abs() * dc.size) & 0xFF_FFFF) >> double;
    coefs[0] = (if levels[0] < 0 { -magnitude } else { magnitude }).clamp(-32768, 32767);
}

fn burden(levels: &[i32], coefs: &[i32], back: &[i32], order: &[u16], lambda: i64) -> i64 {
    let mut bits = 0i64;
    let mut last = 0usize;
    for (index, &at) in order.iter().enumerate() {
        let level = levels[usize::from(at)].unsigned_abs();
        if level != 0 {
            bits += 3 + 2 * i64::from(32 - level.leading_zeros());
            last = index + 1;
        }
    }
    bits += last as i64 / 2;
    let error: i64 = coefs.iter().zip(back).map(|(&a, &b)| i64::from(a - b) * i64::from(a - b)).sum();
    error + lambda * bits
}

fn pick(frame: &Source<'_>, kept: &mut [[[u8; 1024]; 2]], planes: Range<usize>, edges: &[Edges], at: usize, size: usize, list: &[Choice]) -> (Choice, usize, [u32; 2]) {
    let mut best = (u32::MAX, PLAIN, [0; 2]);
    let mut side = 0;
    for &choice in list {
        let mut costs = [0u32; 2];
        for (((plane, edges), pair), cost) in planes.clone().zip(edges).zip(kept.iter_mut()).zip(&mut costs) {
            let trial = &mut pair[side ^ 1][..size * size];
            predict(choice.mode, choice.turn, edges, size, trial);
            *cost = distance(trial, &frame.planes[plane], frame.stride, at, size);
        }
        let total = costs[0] + costs[1];
        if total < best.0 {
            best = (total, choice, costs);
            side ^= 1;
            if total == 0 {
                break;
            }
        }
    }
    (best.1, side, best.2)
}

#[derive(Clone, Copy)]
pub(super) struct Pick {
    pub tone: Choice,
    pub shade: Choice,
    pub sides: [usize; 3],
    pub costs: [u32; 3],
    pub motion: Option<[i32; 2]>,
}

impl Tile<'_> {
    pub(super) fn within(&self, x4: usize, y4: usize, dx4: usize, dy4: usize, piece4: usize, span: usize) -> Reach {
        let mut reach = self.reach(x4 + dx4, y4 + dy4, piece4);
        reach.above_right |= dy4 > 0 && dx4 + piece4 < span;
        reach
    }

    pub(super) fn flood(&mut self, x4: usize, y4: usize, level: u32, tone: Choice, shade: Choice) {
        let size = 1usize << level;
        let stride = self.frame.stride;
        for plane in 0..self.frame.planes.len() {
            let piece = if plane > 0 { size.min(32) } else { size };
            let choice = if plane > 0 { shade } else { tone };
            for (dx, dy) in (0..size).step_by(piece).flat_map(|dy| (0..size).step_by(piece).map(move |dx| (dx, dy))) {
                let reach = self.within(x4, y4, dx / 4, dy / 4, piece / 4, size / 4);
                let (x, y) = (x4 * 4 + dx, y4 * 4 + dy);
                let edges = Edges::gather(&self.recon[plane], stride, x, y, piece, &reach);
                let trial = &mut self.scratch.trial[..piece * piece];
                predict(choice.mode, choice.turn, &edges, piece, trial);
                for (row, line) in trial.chunks_exact(piece).enumerate() {
                    let at = (y + row) * stride + x;
                    self.recon[plane][at..at + piece].copy_from_slice(line);
                }
            }
        }
    }

    pub(super) fn quiet(&self, size: usize) -> u32 {
        (self.ac[0].size as u32 * (64 - self.tuning.bias) * size.min(32) as u32) >> 10
    }

    pub(super) fn still(&mut self, x4: usize, y4: usize) -> bool {
        let stride = self.frame.stride;
        let at = y4 * 4 * stride + x4 * 4;
        let limit = self.quiet(64);
        let reach = self.reach(x4, y4, 16);
        let edges = Edges::gather(&self.recon[0], stride, x4 * 4, y4 * 4, 64, &reach);
        self.draft.motion = None;
        let mut best = (u32::MAX, PLAIN);
        for mode in [DC, VERTICAL, HORIZONTAL, PAETH, SMOOTH] {
            if best.0 == 0 {
                break;
            }
            predict(mode, LEVEL, &edges, 64, &mut self.scratch.trial);
            let cost = distance(&self.scratch.trial, &self.frame.planes[0], stride, at, 64);
            if cost < best.0 {
                best = (cost, Choice { mode, turn: LEVEL });
            }
        }
        if best.0 > limit {
            return false;
        }
        self.flood(x4, y4, 6, best.1, best.1);
        for plane in 1..self.frame.planes.len() {
            let recon = &self.recon[plane];
            let source = &self.frame.planes[plane];
            let cost: u32 = (0..64).map(|row| distance(&recon[at + row * stride..at + row * stride + 64], source, stride, at + row * stride, 64)).sum();
            if cost > limit {
                return false;
            }
        }
        self.draft.skip = true;
        self.draft.tone = best.1;
        self.draft.shade = best.1;
        true
    }

    pub(super) fn work(&mut self, x4: usize, y4: usize, level: u32) {
        let pick = self.choose(x4, y4, level);
        self.code(x4, y4, level, &pick);
    }

    pub(super) fn choose(&mut self, x4: usize, y4: usize, level: u32) -> Pick {
        let size = 1usize << level;
        let (x, y) = (x4 * 4, y4 * 4);
        let reach = self.reach(x4, y4, size / 4);
        let Self { frame, tuning, recon, guesses, .. } = self;
        let stride = frame.stride;
        let at = y * stride + x;
        let (first, rest) = guesses.kept.split_at_mut(1);
        let mut list = [PLAIN; 12];
        let mut count = 0;
        for &mode in &STEADY[..tuning.modes] {
            list[count] = Choice { mode, turn: LEVEL };
            count += 1;
        }
        if let Some(angle) = lean(&frame.planes[0], stride, at, size).filter(|_| tuning.turns > 0) {
            let (mode, turn) = nearest(angle, level >= 3);
            for &turn in &[turn, turn - 1, turn + 1][..tuning.turns] {
                let choice = Choice { mode, turn };
                if (-3..=3).contains(&turn) && (level >= 3 || turn == 0) && !list[..count].contains(&choice) {
                    list[count] = choice;
                    count += 1;
                }
            }
        }
        let edges = [Edges::gather(&recon[0], stride, x, y, size, &reach)];
        let (tone, tone_side, tone_cost) = pick(frame, first, 0..1, &edges, at, size, &list[..count]);
        let (mut shade, mut shade_side, mut shade_cost) = (PLAIN, 0, [0; 2]);
        if frame.planes.len() > 1 {
            let tints = [Edges::gather(&recon[1], stride, x, y, size, &reach), Edges::gather(&recon[2], stride, x, y, size, &reach)];
            let mut list = [tone; 4];
            let mut count = 1;
            for &mode in &TINTS[..tuning.tints] {
                if mode != tone.mode {
                    list[count] = Choice { mode, turn: LEVEL };
                    count += 1;
                }
            }
            (shade, shade_side, shade_cost) = pick(frame, rest, 1..3, &tints, at, size, &list[..count]);
        }
        Pick { tone, shade, sides: [tone_side, shade_side, shade_side], costs: [tone_cost[0], shade_cost[0], shade_cost[1]], motion: None }
    }

    pub(super) fn code(&mut self, x4: usize, y4: usize, level: u32, pick: &Pick) {
        let size = 1usize << level;
        let area = size * size;
        let (x, y) = (x4 * 4, y4 * 4);
        let limit = self.quiet(size);
        let cells = size.div_ceil(8);
        let mut hollow = [false; 16];
        if pick.motion.is_some() && level > 3 {
            for (index, flag) in hollow[..cells * cells].iter_mut().enumerate() {
                *flag = self.survey.void(x4 + (index % cells) * 2, y4 + (index / cells) * 2, 3) == VOID;
            }
        }
        let Self { frame, tuning, recon, draft, guesses, scratch, dc, ac, .. } = self;
        let stride = frame.stride;
        let at = y * stride + x;
        let planes = frame.planes.len();
        let double = u32::from(size == 32);
        let moved = pick.motion.is_some();
        let flat = tuning.plain || (moved && tuning.trial);
        let bends: &[u8] = if moved && size <= 16 && tuning.bend { &BENDS } else { &[] };
        let mut shaped = None;
        if !bends.is_empty() && pick.costs[0] > limit {
            let coded: Vec<usize> = (0..planes).filter(|&plane| pick.costs[plane] > limit).collect();
            for &plane in &coded {
                difference(&mut scratch.shapes[plane][..area], &frame.planes[plane][at..], &guesses.moved[plane][..area], stride, size, &hollow);
            }
            let mut best = (i64::MAX, COSINE);
            for &shape in [COSINE].iter().chain(bends).chain(flat.then_some(&IDENTITY)) {
                let mut total = 0i64;
                for &plane in &coded {
                    let (dc, ac) = (&dc[plane.min(1)], &ac[plane.min(1)]);
                    let (other, back, plain) = (&mut scratch.other[..area], &mut scratch.back[..area], &mut scratch.plain[..area]);
                    transform::forward(&scratch.shapes[plane][..area], other, size, shape);
                    let any = quantize(other, plain, dc, ac, tuning.bias, double);
                    if plane == 0 && !any && shape != COSINE {
                        total = i64::MAX;
                        break;
                    }
                    restore(plain, back, dc, ac, double);
                    total += burden(plain, other, back, scan(size), i64::from(ac.size) * i64::from(ac.size) / BEND_SHARE);
                }
                if total < best.0 {
                    best = (total, shape);
                }
            }
            shaped = Some(best.1);
        }
        let residual = &mut scratch.residual[..area];
        let coefs = &mut scratch.coefs[..area];
        let mut skip = true;
        let mut luma = None;
        for plane in 0..planes {
            let choice = if plane > 0 { pick.shade } else { pick.tone };
            let trial = if pick.motion.is_some() { &guesses.moved[plane][..area] } else { &guesses.kept[plane][pick.sides[plane]][..area] };
            let source = &frame.planes[plane];
            let levels = &mut draft.levels[plane][..area];
            let (dc, ac) = (&dc[plane.min(1)], &ac[plane.min(1)]);
            let mut kind = if moved {
                if plane == 0 { shaped.unwrap_or(COSINE) } else { luma.unwrap_or(COSINE) }
            } else if size == 32 {
                COSINE
            } else {
                MODE_TRANSFORM[usize::from(choice.mode)]
            };
            let mut coded = false;
            if pick.costs[plane] > limit {
                difference(residual, &source[at..], trial, stride, size, &hollow);
                transform::forward(residual, coefs, size, kind);
                coded = quantize(coefs, levels, dc, ac, tuning.bias, double);
                if coded && plane == 0 && size <= 16 && shaped.is_none() && flat {
                    let lambda = i64::from(ac.size) * i64::from(ac.size);
                    let (other, back) = (&mut scratch.other[..area], &mut scratch.back[..area]);
                    let plain = &mut scratch.plain[..area];
                    restore(levels, back, dc, ac, double);
                    let usual = burden(levels, coefs, back, scan(size), lambda);
                    transform::forward(residual, other, size, IDENTITY);
                    quantize(other, plain, dc, ac, tuning.bias, double);
                    restore(plain, back, dc, ac, double);
                    if burden(plain, other, back, scan(size), lambda) < usual {
                        kind = IDENTITY;
                        levels.copy_from_slice(plain);
                        coded = levels.iter().any(|&level| level != 0);
                    }
                }
                if coded {
                    restore(levels, coefs, dc, ac, double);
                    transform::inverse(coefs, residual, size, kind);
                }
            }
            if plane == 0 && coded {
                luma = Some(kind);
            }
            draft.kinds[plane] = kind;
            skip &= !coded;
            let recon = &mut recon[plane];
            if coded {
                for (row, (line, guess)) in residual.chunks_exact(size).zip(trial.chunks_exact(size)).enumerate() {
                    let out = &mut recon[at + row * stride..at + row * stride + size];
                    for ((value, &delta), &base) in out.iter_mut().zip(line).zip(guess) {
                        *value = (i32::from(base) + delta).clamp(0, 255) as u8;
                    }
                }
            } else {
                levels.fill(0);
                for (row, guess) in trial.chunks_exact(size).enumerate() {
                    recon[at + row * stride..at + row * stride + size].copy_from_slice(guess);
                }
            }
        }
        draft.skip = skip;
        draft.tone = pick.tone;
        draft.shade = pick.shade;
        draft.motion = pick.motion;
    }
}
