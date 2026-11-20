mod arith;
mod cost;
mod planes;
mod predict;
mod quant;
mod rate;
mod tables;
mod tokens;
mod transform;
mod trellis;

use arith::BoolWriter;
use cost::{CHROMA_COSTS, CHROMA_PATHS, LARGE_COSTS, LARGE_PATHS, SMALL_COSTS, SMALL_PATHS};
use planes::Planes;
use predict::{ALIKE, Edge, FLAT, SPLIT};
use quant::{Quantizer, Steps};
use rate::Rates;
use tables::{AC_STEPS, BLOCK_MODE_PROBS, CHROMA_MODE_PROBS, DEFAULT_PROBS, LUMA_MODE_PROBS};
use tokens::Tokens;
use trellis::Lane;

const SECOND: usize = 24;
const ANCHORS: [(i32, i32); 6] = [(1, 104), (20, 66), (50, 35), (80, 18), (95, 2), (100, 0)];
const BLUE: usize = 16;
const ROUNDS: usize = 2;
const LONGEST: usize = 16383;
const ROOMIEST: usize = (1 << 19) - 1;
const SCALES: [u32; 4] = [64, 85, 171, 346];
pub(super) const FLATTENING: u8 = 95;
const SMALL_SHIFTS: [(i32, i32); 6] = [(1, 0), (20, 0), (50, 4), (80, 3), (95, 2), (100, 0)];
const SMALL_CANVAS: f64 = 13.0;
const LARGE_CANVAS: f64 = 18.0;
pub(super) const THOROUGH_EFFORT: u8 = 30;
const WIDE_EFFORT: u8 = 70;
const NARROW: usize = 4;
const WIDE: usize = 10;

#[derive(Clone)]
struct Macro {
    segment: u8,
    luma: u8,
    chroma: u8,
    small: [u8; 16],
    coded: bool,
    levels: [[i16; 16]; 25],
}

struct Canvas {
    data: Vec<u8>,
    stride: usize,
}

impl Canvas {
    fn new(width: usize, height: usize, spare: usize) -> Self {
        let stride = width + 1 + spare;
        let mut data = vec![127u8; stride * (height + 1)];
        for row in 0..height {
            data[(row + 1) * stride] = 129;
        }
        Self { data, stride }
    }

    #[inline]
    fn origin(&self, x: usize, y: usize) -> usize {
        (y + 1) * self.stride + x + 1
    }

    fn column(&self, origin: usize, out: &mut [u8]) {
        for (row, value) in out.iter_mut().enumerate() {
            *value = self.data[origin + row * self.stride - 1];
        }
    }

    fn store(&mut self, origin: usize, size: usize, pixels: &[u8]) {
        for (row, line) in pixels.chunks_exact(size).enumerate() {
            let at = origin + row * self.stride;
            self.data[at..at + size].copy_from_slice(line);
        }
    }
}

#[inline]
fn gap(a: &[u8], b: &[u8]) -> u32 {
    a.iter().zip(b).map(|(a, b)| {
        let delta = i16::from(*a) - i16::from(*b);
        i32::from(delta) * i32::from(delta)
    }).sum::<i32>() as u32
}

#[inline]
fn cut(source: &[u8], stride: usize, x: usize, y: usize) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (row, line) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let at = (y + row) * stride + x;
        line.copy_from_slice(&source[at..at + 4]);
    }
    out
}

#[inline]
fn paste(target: &mut [u8], stride: usize, x: usize, y: usize, block: &[u8; 16]) {
    for (row, line) in block.as_chunks::<4>().0.iter().enumerate() {
        let at = (y + row) * stride + x;
        target[at..at + 4].copy_from_slice(line);
    }
}

#[inline]
fn residual(source: &[u8; 16], predicted: &[u8; 16]) -> [i16; 16] {
    std::array::from_fn(|index| i16::from(source[index]) - i16::from(predicted[index]))
}

#[inline]
fn rebuild(predicted: &[u8; 16], restored: &[i16; 16]) -> [u8; 16] {
    let delta = transform::inverse(restored);
    std::array::from_fn(|index| (i16::from(predicted[index]) + delta[index]).clamp(0, 255) as u8)
}

#[inline]
fn settle(source: &[u8; 16], predicted: &[u8; 16], steps: &Steps) -> ([i16; 16], [u8; 16], bool) {
    settle_lane(source, predicted, steps, None)
}

#[inline]
fn settle_lane(source: &[u8; 16], predicted: &[u8; 16], steps: &Steps, lane: Option<&Lane<'_>>) -> ([i16; 16], [u8; 16], bool) {
    let coefficients = transform::forward(&residual(source, predicted));
    let (levels, any) = lane.map_or_else(|| steps.quantize(&coefficients, 0), |lane| trellis::quantize(&coefficients, steps, 0, lane));
    let rebuilt = if any { rebuild(predicted, &steps.restore(&levels)) } else { *predicted };
    (levels, rebuilt, any)
}

#[derive(Clone, Copy)]
struct Tools {
    thorough: bool,
    breadth: usize,
    texture: bool,
}

impl Tools {
    fn new(effort: u8, quality: u8) -> Self {
        let wide = effort >= WIDE_EFFORT && quality < FLATTENING;
        Self { thorough: effort >= THOROUGH_EFFORT || quality >= FLATTENING, breadth: if wide { WIDE } else { NARROW }, texture: wide }
    }
}

fn chroma_pass(planes: &Planes, quantizers: &[Quantizer; 4], macros: &mut [Macro], rates: &Rates, tools: Tools) {
    let thorough = tools.thorough;
    let mut over_marks = vec![[false; 4]; planes.columns];
    let (columns, rows) = (planes.columns, planes.rows);
    let stride = columns * 8;
    let mut canvases = [Canvas::new(stride, rows * 8, 0), Canvas::new(stride, rows * 8, 0)];
    let sources = [&planes.blue, &planes.red];
    for row in 0..rows {
        let mut side_marks = [false; 4];
        for (column, over) in over_marks.iter_mut().enumerate() {
            let index = row * columns + column;
            let origin = canvases[0].origin(column * 8, row * 8);
            let span = canvases[0].stride;
            let mut lefts = [[0u8; 8]; 2];
            for (canvas, left) in canvases.iter().zip(&mut lefts) {
                canvas.column(origin, left);
            }
            let edges: [Edge<'_>; 2] = std::array::from_fn(|plane| Edge { top: &canvases[plane].data[origin - span..origin - span + 8], left: &lefts[plane], corner: canvases[plane].data[origin - span - 1], has_top: row > 0, has_left: column > 0 });
            let mut predicted = [[0u8; 64]; 2];
            let quantizer = &quantizers[usize::from(macros[index].segment)];
            if planes.peaks[index] == 0 {
                for plane in 0..2 {
                    predict::large::<8>(FLAT, &edges[plane], &mut predicted[plane]);
                }
                for plane in 0..2 {
                    canvases[plane].store(origin, 8, &predicted[plane]);
                }
                *over = [false; 4];
                side_marks = [false; 4];
                continue;
            }
            let mut source = [[0u8; 64]; 2];
            for plane in 0..2 {
                for y in 0..8 {
                    let at = (row * 8 + y) * stride + column * 8;
                    source[plane][y * 8..y * 8 + 8].copy_from_slice(&sources[plane][at..at + 8]);
                }
            }
            if thorough {
                let mut best: Option<(u64, usize)> = None;
                let mut trial = [[0u8; 64]; 2];
                for (candidate, &cost) in CHROMA_COSTS.iter().enumerate() {
                    let mut rate = u64::from(cost);
                    let mut distortion = 0u64;
                    let (mut top, mut side) = (*over, side_marks);
                    for plane in 0..2 {
                        predict::large::<8>(candidate, &edges[plane], &mut trial[plane]);
                        let mut rebuilt = [0u8; 64];
                        for block in 0..4 {
                            let (x, y) = (block % 2 * 4, block / 2 * 4);
                            let (across, down) = (plane * 2 + block % 2, plane * 2 + block / 2);
                            let (found, pixels, any) = settle(&cut(&source[plane], 8, x, y), &cut(&trial[plane], 8, x, y), &quantizer.chroma);
                            rate += u64::from(rates.block(&found, 0, 2, usize::from(top[across]) + usize::from(side[down])));
                            top[across] = any;
                            side[down] = any;
                            paste(&mut rebuilt, 8, x, y, &pixels);
                        }
                        distortion += u64::from(gap(&source[plane], &rebuilt));
                    }
                    let score = rate * quantizer.tint + (distortion << 8);
                    if best.is_none_or(|kept| score < kept.0) {
                        best = Some((score, candidate));
                    }
                }
                let mode = best.map_or(FLAT, |(_, mode)| mode);
                let (mut top, mut side) = (*over, side_marks);
                let entry = &mut macros[index];
                entry.chroma = mode as u8;
                let mut rebuilt = [[0u8; 64]; 2];
                for plane in 0..2 {
                    predict::large::<8>(mode, &edges[plane], &mut trial[plane]);
                    for block in 0..4 {
                        let (x, y) = (block % 2 * 4, block / 2 * 4);
                        let (across, down) = (plane * 2 + block % 2, plane * 2 + block / 2);
                        let lane = Lane { rates, kind: 2, context: usize::from(top[across]) + usize::from(side[down]), lambda: quantizer.trellis_tint };
                        let (found, pixels, any) = settle_lane(&cut(&source[plane], 8, x, y), &cut(&trial[plane], 8, x, y), &quantizer.chroma, Some(&lane));
                        top[across] = any;
                        side[down] = any;
                        entry.levels[BLUE + plane * 4 + block] = found;
                        entry.coded |= any;
                        paste(&mut rebuilt[plane], 8, x, y, &pixels);
                    }
                }
                for plane in 0..2 {
                    canvases[plane].store(origin, 8, &rebuilt[plane]);
                }
                *over = top;
                side_marks = side;
                continue;
            }
            let mut best = u64::MAX;
            let mut mode = FLAT;
            let mut trial = [[0u8; 64]; 2];
            for (candidate, &cost) in CHROMA_COSTS.iter().enumerate() {
                let mut score = quantizer.lambda * u64::from(cost);
                for plane in 0..2 {
                    predict::large::<8>(candidate, &edges[plane], &mut trial[plane]);
                    score += u64::from(gap(&source[plane], &trial[plane])) << 8;
                }
                if score < best {
                    best = score;
                    mode = candidate;
                    predicted = trial;
                }
            }
            let entry = &mut macros[index];
            entry.chroma = mode as u8;
            let mut rebuilt = [[0u8; 64]; 2];
            for plane in 0..2 {
                for block in 0..4 {
                    let (x, y) = (block % 2 * 4, block / 2 * 4);
                    let (levels, pixels, any) = settle(&cut(&source[plane], 8, x, y), &cut(&predicted[plane], 8, x, y), &quantizer.chroma);
                    entry.levels[BLUE + plane * 4 + block] = levels;
                    entry.coded |= any;
                    paste(&mut rebuilt[plane], 8, x, y, &pixels);
                }
            }
            for plane in 0..2 {
                canvases[plane].store(origin, 8, &rebuilt[plane]);
            }
        }
    }
}

struct Whole {
    mode: usize,
    levels: [[i16; 16]; 17],
    pixels: [u8; 256],
    coded: bool,
}

struct Parts {
    modes: [u8; 16],
    levels: [[i16; 16]; 16],
    pixels: [u8; 256],
    score: u64,
    coded: bool,
}

struct Nearby {
    over: [u8; 4],
    side: [u8; 4],
}

fn pick(source: &[u8; 256], edge: &Edge<'_>, quantizer: &Quantizer) -> (usize, [u8; 256], u64) {
    let mut best = u64::MAX;
    let mut mode = FLAT;
    let mut predicted = [0u8; 256];
    let mut trial = [0u8; 256];
    for (candidate, &cost) in LARGE_COSTS[..SPLIT].iter().enumerate() {
        predict::large::<16>(candidate, edge, &mut trial);
        let score = (u64::from(gap(source, &trial)) << 8) + quantizer.lambda * u64::from(cost);
        if score < best {
            best = score;
            mode = candidate;
            predicted = trial;
        }
    }
    (mode, predicted, best)
}

fn whole(source: &[u8; 256], mode: usize, predicted: &[u8; 256], quantizer: &Quantizer, lane: Option<(&Rates, [bool; 4], [bool; 4])>) -> Whole {
    let (mut above, mut beside) = lane.map_or(([false; 4], [false; 4]), |(_, above, beside)| (above, beside));
    let mut coefficients = [[0i16; 16]; 16];
    let mut flats = [0i16; 16];
    for block in 0..16 {
        let (x, y) = (block % 4 * 4, block / 4 * 4);
        coefficients[block] = transform::forward(&residual(&cut(source, 16, x, y), &cut(predicted, 16, x, y)));
        flats[block] = coefficients[block][0];
    }
    let mut out = Whole { mode, levels: [[0; 16]; 17], pixels: [0; 256], coded: false };
    let (levels, second) = quantizer.second.quantize(&transform::forward_walsh(&flats), 0);
    out.levels[16] = levels;
    out.coded = second;
    let flats = transform::inverse_walsh(&quantizer.second.restore(&levels));
    for block in 0..16 {
        let (x, y) = (block % 4, block / 4);
        let (levels, any) = match lane {
            Some((rates, _, _)) => trellis::quantize(&coefficients[block], &quantizer.luma, 1, &Lane { rates, kind: 0, context: usize::from(above[x]) + usize::from(beside[y]), lambda: quantizer.trellis_large }),
            None => quantizer.luma.quantize(&coefficients[block], 1),
        };
        above[x] = any;
        beside[y] = any;
        out.coded |= any;
        out.levels[block] = levels;
        let before = cut(predicted, 16, x * 4, y * 4);
        let pixels = if any {
            let mut restored = quantizer.luma.restore(&levels);
            restored[0] = flats[block];
            rebuild(&before, &restored)
        } else {
            let lift = (flats[block] + 4) >> 3;
            std::array::from_fn(|index| (i16::from(before[index]) + lift).clamp(0, 255) as u8)
        };
        paste(&mut out.pixels, 16, x * 4, y * 4, &pixels);
    }
    out
}

fn parts(source: &[u8; 256], edge: &Edge<'_>, quantizer: &Quantizer, nearby: &Nearby, limit: u64) -> Option<Parts> {
    let mut out = Parts { modes: [0; 16], levels: [[0; 16]; 16], pixels: [0; 256], score: quantizer.lambda * u64::from(LARGE_COSTS[SPLIT]) + quantizer.toll, coded: false };
    for block in 0..16 {
        let (x, y) = (block % 4, block / 4);
        let mut top = [0u8; 8];
        let mut left = [0u8; 4];
        let lifted = usize::saturating_sub(y * 4, 1) * 16 + x * 4;
        let corner = match (x, y) {
            (0, 0) => edge.corner,
            (_, 0) => edge.top[x * 4 - 1],
            (0, _) => edge.left[y * 4 - 1],
            _ => out.pixels[lifted - 1],
        };
        if y == 0 {
            top.copy_from_slice(&edge.top[x * 4..x * 4 + 8]);
        } else {
            top[..4].copy_from_slice(&out.pixels[lifted..lifted + 4]);
            if x < 3 {
                top[4..].copy_from_slice(&out.pixels[lifted + 4..lifted + 8]);
            } else {
                top[4..].copy_from_slice(&edge.top[16..20]);
            }
        }
        for (row, value) in left.iter_mut().enumerate() {
            *value = if x == 0 { edge.left[y * 4 + row] } else { out.pixels[(y * 4 + row) * 16 + x * 4 - 1] };
        }
        let over = if y == 0 { nearby.over[x] } else { out.modes[block - 4] };
        let side = if x == 0 { nearby.side[y] } else { out.modes[block - 1] };
        let costs = &SMALL_COSTS[usize::from(over)][usize::from(side)];
        let wanted = cut(source, 16, x * 4, y * 4);
        let mut best = u64::MAX;
        let mut mode = 0;
        let mut chosen = [0u8; 16];
        let mut closest = u32::MAX;
        for (candidate, predicted) in predict::plain(&top, &left, corner).iter().enumerate() {
            let miss = gap(&wanted, predicted);
            let score = (u64::from(miss) << 8) + quantizer.lambda * u64::from(costs[candidate]);
            closest = closest.min(miss);
            if score < best {
                best = score;
                mode = candidate;
                chosen = *predicted;
            }
        }
        if closest > 0 {
            for (candidate, predicted) in predict::slanted(&top, &left, corner).iter().enumerate() {
                let score = (u64::from(gap(&wanted, predicted)) << 8) + quantizer.lambda * u64::from(costs[candidate + 4]);
                if score < best {
                    best = score;
                    mode = candidate + 4;
                    chosen = *predicted;
                }
            }
        }
        out.score += best;
        if out.score >= limit {
            return None;
        }
        let (levels, pixels, any) = settle(&wanted, &chosen, &quantizer.luma);
        out.coded |= any;
        out.modes[block] = mode as u8;
        out.levels[block] = levels;
        paste(&mut out.pixels, 16, x * 4, y * 4, &pixels);
    }
    Some(out)
}


const WEIGHTS: [i32; 16] = [38, 32, 20, 9, 32, 28, 17, 7, 20, 17, 10, 4, 9, 7, 4, 2];
const FLAT_LIMIT: usize = 3;
const FLAT_PENALTY: u64 = 140;
const SPLIT_HEADER: u64 = 211;

#[inline]
fn texture(block: &[u8; 16]) -> i32 {
    let mut temp = [0i32; 16];
    for (row, out) in block.as_chunks::<4>().0.iter().zip(temp.as_chunks_mut::<4>().0) {
        let [p0, p1, p2, p3] = row.map(i32::from);
        let (a0, a1, a2, a3) = (p0 + p2, p1 + p3, p1 - p3, p0 - p2);
        *out = [a0 + a1, a3 + a2, a3 - a2, a0 - a1];
    }
    let mut sum = 0;
    for column in 0..4 {
        let (a0, a1) = (temp[column] + temp[8 + column], temp[4 + column] + temp[12 + column]);
        let (a2, a3) = (temp[4 + column] - temp[12 + column], temp[column] - temp[8 + column]);
        sum += WEIGHTS[column] * (a0 + a1).abs() + WEIGHTS[4 + column] * (a3 + a2).abs() + WEIGHTS[8 + column] * (a3 - a2).abs() + WEIGHTS[12 + column] * (a0 - a1).abs();
    }
    sum
}

#[inline]
fn warp(quantizer: &Quantizer, source: i32, rebuilt: &[u8; 16]) -> u64 {
    let gap = u64::from((texture(rebuilt) - source).unsigned_abs() >> 5);
    (quantizer.texture * gap + 128) >> 8
}

fn flat(levels: &[i16; 16]) -> bool {
    levels[1..].iter().filter(|&&level| level != 0).count() <= FLAT_LIMIT
}

#[derive(Clone, Copy)]
struct Marks {
    above: [bool; 4],
    beside: [bool; 4],
}

struct Coder<'a> {
    quantizer: &'a Quantizer,
    rates: &'a Rates,
    tools: Tools,
}

struct Decided {
    whole: Whole,
    score: u64,
    marks: Marks,
    dc: bool,
}

fn rd_large(source: &[u8; 256], edge: &Edge<'_>, coder: &Coder<'_>, marks: Marks, context: usize) -> Decided {
    let (quantizer, rates) = (coder.quantizer, coder.rates);
    let textures: Option<[i32; 16]> = coder.tools.texture.then(|| std::array::from_fn(|block| texture(&cut(source, 16, block % 4 * 4, block / 4 * 4))));
    let mut predicted = [0u8; 256];
    let mut best = (u64::MAX, FLAT, u64::MAX);
    for (mode, &cost) in LARGE_COSTS[..SPLIT].iter().enumerate() {
        predict::large::<16>(mode, edge, &mut predicted);
        let candidate = whole(source, mode, &predicted, quantizer, None);
        let mut distortion = u64::from(gap(source, &candidate.pixels));
        let mut rate = u64::from(cost) + u64::from(rates.block(&candidate.levels[16], 0, 1, context));
        let (mut top, mut side) = (marks.above, marks.beside);
        for block in 0..16 {
            let (x, y) = (block % 4, block / 4);
            distortion += textures.map_or(0, |textures| warp(quantizer, textures[block], &cut(&candidate.pixels, 16, x * 4, y * 4)));
            rate += u64::from(rates.block(&candidate.levels[block], 1, 0, usize::from(top[x]) + usize::from(side[y])));
            let mark = candidate.levels[block][1..].iter().any(|&level| level != 0);
            top[x] = mark;
            side[y] = mark;
        }
        let score = rate * quantizer.large + (distortion << 8);
        if score < best.0 {
            best = (score, mode, rate * quantizer.mode + (distortion << 8));
        }
    }
    let (_, mode, score) = best;
    predict::large::<16>(mode, edge, &mut predicted);
    let chosen = whole(source, mode, &predicted, quantizer, Some((rates, marks.above, marks.beside)));
    let (mut top, mut side) = (marks.above, marks.beside);
    for block in 0..16 {
        let mark = chosen.levels[block][1..].iter().any(|&level| level != 0);
        top[block % 4] = mark;
        side[block / 4] = mark;
    }
    let dc = chosen.levels[16].iter().any(|&level| level != 0);
    Decided { whole: chosen, score, marks: Marks { above: top, beside: side }, dc }
}

fn rd_small(source: &[u8; 256], edge: &Edge<'_>, coder: &Coder<'_>, nearby: &Nearby, marks: Marks, limit: u64) -> Option<(Parts, Marks)> {
    let (quantizer, rates, tools) = (coder.quantizer, coder.rates, coder.tools);
    let mut out = Parts { modes: [0; 16], levels: [[0; 16]; 16], pixels: [0; 256], score: SPLIT_HEADER * quantizer.mode, coded: false };
    let (mut top_marks, mut side_marks) = (marks.above, marks.beside);
    for block in 0..16 {
        let (x, y) = (block % 4, block / 4);
        let mut top = [0u8; 8];
        let mut left = [0u8; 4];
        let lifted = usize::saturating_sub(y * 4, 1) * 16 + x * 4;
        let corner = match (x, y) {
            (0, 0) => edge.corner,
            (_, 0) => edge.top[x * 4 - 1],
            (0, _) => edge.left[y * 4 - 1],
            _ => out.pixels[lifted - 1],
        };
        if y == 0 {
            top.copy_from_slice(&edge.top[x * 4..x * 4 + 8]);
        } else {
            top[..4].copy_from_slice(&out.pixels[lifted..lifted + 4]);
            if x < 3 {
                top[4..].copy_from_slice(&out.pixels[lifted + 4..lifted + 8]);
            } else {
                top[4..].copy_from_slice(&edge.top[16..20]);
            }
        }
        for (row, value) in left.iter_mut().enumerate() {
            *value = if x == 0 { edge.left[y * 4 + row] } else { out.pixels[(y * 4 + row) * 16 + x * 4 - 1] };
        }
        let over = if y == 0 { nearby.over[x] } else { out.modes[block - 4] };
        let side = if x == 0 { nearby.side[y] } else { out.modes[block - 1] };
        let costs = &SMALL_COSTS[usize::from(over)][usize::from(side)];
        let wanted = cut(source, 16, x * 4, y * 4);
        let wanted_texture = tools.texture.then(|| texture(&wanted));
        let plain = predict::plain(&top, &left, corner);
        let slanted = predict::slanted(&top, &left, corner);
        let context = usize::from(top_marks[x]) + usize::from(side_marks[y]);
        let mut best: Option<(u64, u64, u64, usize)> = None;
        let mut order: [(u64, usize); 10] = std::array::from_fn(|candidate| {
            let predicted = if candidate < 4 { &plain[candidate] } else { &slanted[candidate - 4] };
            ((u64::from(gap(&wanted, predicted)) << 8) + quantizer.lambda * u64::from(costs[candidate]), candidate)
        });
        order.sort_unstable();
        for &(_, candidate) in &order[..tools.breadth] {
            let predicted = if candidate < 4 { &plain[candidate] } else { &slanted[candidate - 4] };
            let (levels, pixels, _) = settle(&wanted, predicted, &quantizer.luma);
            let distortion = u64::from(gap(&wanted, &pixels)) + wanted_texture.map_or(0, |source| warp(quantizer, source, &pixels));
            let header = u64::from(costs[candidate]);
            let mut rate = if candidate > 0 && flat(&levels) { FLAT_PENALTY } else { 0 };
            if best.is_some_and(|kept| (header + rate) * quantizer.lambda + (distortion << 8) >= kept.0) {
                continue;
            }
            rate += u64::from(rates.block(&levels, 0, 3, context));
            let score = (header + rate) * quantizer.lambda + (distortion << 8);
            if best.is_none_or(|kept| score < kept.0) {
                best = Some((score, header + rate, distortion, candidate));
            }
        }
        let (_, rate, distortion, mode) = best?;
        let lane = Lane { rates, kind: 3, context, lambda: quantizer.trellis_small };
        let predicted = if mode < 4 { &plain[mode] } else { &slanted[mode - 4] };
        let (levels, pixels, _) = settle_lane(&wanted, predicted, &quantizer.luma, Some(&lane));
        out.score += rate * quantizer.mode + (distortion << 8);
        if out.score >= limit {
            return None;
        }
        let mark = levels.iter().any(|&level| level != 0);
        top_marks[x] = mark;
        side_marks[y] = mark;
        out.coded |= mark;
        out.modes[block] = mode as u8;
        out.levels[block] = levels;
        paste(&mut out.pixels, 16, x * 4, y * 4, &pixels);
    }
    Some((out, Marks { above: top_marks, beside: side_marks }))
}

fn luma_pass(planes: &Planes, quantizers: &[Quantizer; 4], macros: &mut [Macro], rates: &Rates, tools: Tools) {
    let thorough = tools.thorough;
    let mut over_marks = vec![[false; 4]; planes.columns];
    let mut over_dc = vec![false; planes.columns];
    let (columns, rows) = (planes.columns, planes.rows);
    let stride = columns * 16;
    let mut canvas = Canvas::new(stride, rows * 16, 4);
    let mut over_modes = vec![0u8; columns * 4];
    for row in 0..rows {
        let mut side_modes = [0u8; 4];
        let mut side_marks = [false; 4];
        let mut side_dc = false;
        for column in 0..columns {
            let index = row * columns + column;
            let origin = canvas.origin(column * 16, row * 16);
            let span = canvas.stride;
            let mut left = [0u8; 16];
            canvas.column(origin, &mut left);
            let mut top = [0u8; 20];
            top.copy_from_slice(&canvas.data[origin - span..origin - span + 20]);
            let edge = Edge { top: &top, left: &left, corner: canvas.data[origin - span - 1], has_top: row > 0, has_left: column > 0 };
            let modes = &mut over_modes[column * 4..column * 4 + 4];
            let quantizer = &quantizers[usize::from(macros[index].segment)];
            if planes.peaks[index] == 0 {
                let mut predicted = [0u8; 256];
                predict::large::<16>(FLAT, &edge, &mut predicted);
                canvas.store(origin, 16, &predicted);
                modes.fill(ALIKE[FLAT]);
                side_modes = [ALIKE[FLAT]; 4];
                over_marks[column] = [false; 4];
                side_marks = [false; 4];
                over_dc[column] = false;
                side_dc = false;
                continue;
            }
            let mut source = [0u8; 256];
            for y in 0..16 {
                let at = (row * 16 + y) * stride + column * 16;
                source[y * 16..y * 16 + 16].copy_from_slice(&planes.luma[at..at + 16]);
            }
            let mut nearby = Nearby { over: [0; 4], side: side_modes };
            nearby.over.copy_from_slice(modes);
            if thorough {
                let coder = Coder { quantizer, rates, tools };
                let marks = Marks { above: over_marks[column], beside: side_marks };
                let large = rd_large(&source, &edge, &coder, marks, usize::from(over_dc[column]) + usize::from(side_dc));
                let entry = &mut macros[index];
                if let Some((split, found)) = rd_small(&source, &edge, &coder, &nearby, marks, large.score) {
                    entry.luma = SPLIT as u8;
                    entry.small = split.modes;
                    entry.levels[..16].copy_from_slice(&split.levels);
                    entry.coded |= split.coded;
                    canvas.store(origin, 16, &split.pixels);
                    for place in 0..4 {
                        modes[place] = split.modes[12 + place];
                        side_modes[place] = split.modes[place * 4 + 3];
                    }
                    over_marks[column] = found.above;
                    side_marks = found.beside;
                } else {
                    let single = large.whole;
                    entry.luma = single.mode as u8;
                    entry.levels[..16].copy_from_slice(&single.levels[..16]);
                    entry.levels[SECOND] = single.levels[16];
                    entry.coded |= single.coded;
                    canvas.store(origin, 16, &single.pixels);
                    modes.fill(ALIKE[single.mode]);
                    side_modes = [ALIKE[single.mode]; 4];
                    over_marks[column] = large.marks.above;
                    side_marks = large.marks.beside;
                    over_dc[column] = large.dc;
                    side_dc = large.dc;
                }
                continue;
            }
            let (mode, predicted, score) = pick(&source, &edge, quantizer);
            let entry = &mut macros[index];
            if let Some(split) = parts(&source, &edge, quantizer, &nearby, score) {
                entry.luma = SPLIT as u8;
                entry.small = split.modes;
                entry.levels[..16].copy_from_slice(&split.levels);
                entry.coded |= split.coded;
                canvas.store(origin, 16, &split.pixels);
                for place in 0..4 {
                    modes[place] = split.modes[12 + place];
                    side_modes[place] = split.modes[place * 4 + 3];
                }
            } else {
                let single = whole(&source, mode, &predicted, quantizer, None);
                entry.luma = single.mode as u8;
                entry.levels[..16].copy_from_slice(&single.levels[..16]);
                entry.levels[SECOND] = single.levels[16];
                entry.coded |= single.coded;
                canvas.store(origin, 16, &single.pixels);
                modes.fill(ALIKE[single.mode]);
                side_modes = [ALIKE[single.mode]; 4];
            }
        }
        let last = canvas.origin(stride, row * 16 + 15);
        let value = canvas.data[last - 1];
        canvas.data[last..last + 4].fill(value);
    }
}

fn tokenize(planes: &Planes, macros: &[Macro]) -> Tokens {
    let columns = planes.columns;
    let mut tokens = Tokens::new(macros.len() * 64);
    let mut above = vec![[false; 9]; columns];
    for line in macros.chunks_exact(columns) {
        let mut beside = [false; 9];
        for (entry, over) in line.iter().zip(&mut above) {
            let split = usize::from(entry.luma) == SPLIT;
            if !entry.coded {
                over[..8].fill(false);
                beside[..8].fill(false);
                if !split {
                    over[8] = false;
                    beside[8] = false;
                }
                continue;
            }
            if !split {
                let mark = tokens.block(&entry.levels[SECOND], 0, 1, usize::from(over[8]) + usize::from(beside[8]));
                over[8] = mark;
                beside[8] = mark;
            }
            let (first, kind) = if split { (0, 3) } else { (1, 0) };
            for block in 0..16 {
                let (x, y) = (block % 4, block / 4);
                let mark = tokens.block(&entry.levels[block], first, kind, usize::from(over[x]) + usize::from(beside[y]));
                over[x] = mark;
                beside[y] = mark;
            }
            for plane in 0..2 {
                for block in 0..4 {
                    let (x, y) = (4 + plane * 2 + block % 2, 4 + plane * 2 + block / 2);
                    let mark = tokens.block(&entry.levels[BLUE + plane * 4 + block], 0, 2, usize::from(over[x]) + usize::from(beside[y]));
                    over[x] = mark;
                    beside[y] = mark;
                }
            }
        }
    }
    tokens
}

fn follow(header: &mut BoolWriter, path: &[(u8, bool)], probs: &[u8]) {
    for &(node, value) in path {
        header.bit(value, probs[usize::from(node)]);
    }
}

fn modes(header: &mut BoolWriter, columns: usize, macros: &[Macro], skip: Option<u8>, tree: Option<[u8; 3]>) {
    let mut above = vec![0u8; columns * 4];
    for line in macros.chunks_exact(columns) {
        let mut beside = [0u8; 4];
        for (column, entry) in line.iter().enumerate() {
            let over = &mut above[column * 4..column * 4 + 4];
            if let Some(tree) = tree {
                let upper = entry.segment >= 2;
                header.bit(upper, tree[0]);
                header.bit(entry.segment & 1 == 1, tree[1 + usize::from(upper)]);
            }
            if let Some(prob) = skip {
                header.bit(!entry.coded, prob);
            }
            let luma = usize::from(entry.luma);
            follow(header, LARGE_PATHS[luma], &LUMA_MODE_PROBS);
            if luma == SPLIT {
                for block in 0..16 {
                    let (x, y) = (block % 4, block / 4);
                    let mode = entry.small[block];
                    follow(header, SMALL_PATHS[usize::from(mode)], &BLOCK_MODE_PROBS[usize::from(over[x])][usize::from(beside[y])]);
                    over[x] = mode;
                    beside[y] = mode;
                }
            } else {
                over.fill(ALIKE[luma]);
                beside = [ALIKE[luma]; 4];
            }
            follow(header, CHROMA_PATHS[usize::from(entry.chroma)], &CHROMA_MODE_PROBS);
        }
    }
}

fn index_for(quality: u8) -> usize {
    let quality = i32::from(quality.clamp(1, 100));
    let pair = ANCHORS.windows(2).find(|pair| quality <= pair[1].0).unwrap_or(&ANCHORS[ANCHORS.len() - 2..]);
    let (low, high) = (pair[0], pair[1]);
    let span = high.0 - low.0;
    ((low.1 * (high.0 - quality) + high.1 * (quality - low.0) + span / 2) / span).clamp(0, 127) as usize
}

fn coarser(index: usize, scale: u32) -> usize {
    let target = u32::from(AC_STEPS[index]) * scale / 64;
    AC_STEPS.iter().position(|&step| u32::from(step) >= target).unwrap_or(127).max(index)
}

fn scale(quality: u8, segment: usize) -> u32 {
    let reach = u32::from(quality.clamp(FLATTENING, 100) - FLATTENING);
    let span = u32::from(100 - FLATTENING);
    (SCALES[segment] * (span - reach) + SCALES[0] * reach + span / 2) / span
}

fn small_offset(quality: u8, canvas: usize) -> usize {
    let quality = i32::from(quality.clamp(1, 100));
    let pair = SMALL_SHIFTS.windows(2).find(|pair| quality <= pair[1].0).unwrap_or(&SMALL_SHIFTS[SMALL_SHIFTS.len() - 2..]);
    let (low, high) = (pair[0], pair[1]);
    let shift = f64::from(low.1 * (high.0 - quality) + high.1 * (quality - low.0)) / f64::from(high.0 - low.0);
    let size = (canvas.max(1) as f64).log2();
    let weight = ((LARGE_CANVAS - size) / (LARGE_CANVAS - SMALL_CANVAS)).clamp(0.0, 1.0);
    (shift * weight + 0.5) as usize
}

fn smoothing(index: usize) -> u32 {
    if index == 0 { 0 } else { ((index as u32 * 9 + 100) / 20).min(63) }
}

fn ratio(part: usize, whole: usize) -> u8 {
    (part * 255 + whole / 2).checked_div(whole).map_or(255, |share| share.clamp(1, 255) as u8)
}


pub(super) fn encode(rgba: &[u8], alpha: &[u8], width: usize, height: usize, quality: u8, effort: u8, canvas: usize) -> Option<Vec<u8>> {
    if width > LONGEST || height > LONGEST {
        return None;
    }
    let planes = planes::convert(rgba, alpha, width, height, ROUNDS);
    let base = (index_for(quality) + small_offset(quality, canvas)).min(127);
    let shift = (base as i32 * 2).min(8);
    let mut macros = vec![Macro { segment: 0, luma: FLAT as u8, chroma: FLAT as u8, small: [0; 16], coded: false, levels: [[0; 16]; 25] }; planes.columns * planes.rows];
    let mut census = [0usize; 4];
    for (entry, &peak) in macros.iter_mut().zip(&planes.peaks) {
        entry.segment = match peak {
            0 => 4,
            1..=47 => 3,
            48..=95 => 2,
            96..=191 => 1,
            _ => 0,
        };
        if peak > 0 {
            census[usize::from(entry.segment)] += 1;
        }
    }
    let common = (0..4).max_by_key(|&segment| census[segment]).unwrap_or(0);
    let split = census.iter().filter(|&&count| count > 0).count() > 1;
    for entry in &mut macros {
        if entry.segment == 4 || !split {
            entry.segment = common as u8;
        }
    }
    let indices: [usize; 4] = std::array::from_fn(|segment| coarser(base, scale(quality, segment)));
    let quantizers: [Quantizer; 4] = std::array::from_fn(|segment| Quantizer::new(indices[segment], shift));
    let tools = Tools::new(effort, quality);
    let rates = Rates::new(&DEFAULT_PROBS);
    chroma_pass(&planes, &quantizers, &mut macros, &rates, tools);
    luma_pass(&planes, &quantizers, &mut macros, &rates, tools);
    let tokens = tokenize(&planes, &macros);
    let mut header = BoolWriter::new(macros.len() * 4 + 256);
    header.literal(0, 2);
    header.flag(split);
    let mut tree = None;
    if split {
        header.flag(true);
        header.flag(true);
        header.flag(true);
        for index in indices {
            header.flag(true);
            header.literal(index as u32, 7);
            header.flag(false);
        }
        for index in indices {
            header.flag(true);
            header.literal(smoothing(index), 6);
            header.flag(false);
        }
        let mut tally = [0usize; 4];
        for entry in &macros {
            tally[usize::from(entry.segment)] += 1;
        }
        let probs = [ratio(tally[0] + tally[1], macros.len()), ratio(tally[0], tally[0] + tally[1]), ratio(tally[2], tally[2] + tally[3])];
        for prob in probs {
            header.flag(prob != 255);
            if prob != 255 {
                header.literal(u32::from(prob), 8);
            }
        }
        tree = Some(probs);
    }
    let lead = indices[common];
    header.flag(false);
    header.literal(smoothing(lead), 6);
    header.literal(0, 3);
    header.flag(false);
    header.literal(0, 2);
    header.literal(lead as u32, 7);
    header.literal(0, 3);
    for _ in 0..2 {
        header.flag(shift != 0);
        if shift != 0 {
            header.literal(shift.unsigned_abs(), 4);
            header.flag(shift < 0);
        }
    }
    header.flag(false);
    let probs = tokens.settle(&mut header);
    let idle = macros.iter().filter(|entry| !entry.coded).count();
    let skip = (idle > 0).then(|| ratio(macros.len() - idle, macros.len()));
    header.flag(skip.is_some());
    if let Some(prob) = skip {
        header.literal(u32::from(prob), 8);
    }
    modes(&mut header, planes.columns, &macros, skip, tree);
    let first = header.finish();
    if first.len() > ROOMIEST {
        return None;
    }
    let second = tokens.write(&probs);
    let mut out = Vec::with_capacity(10 + first.len() + second.len());
    let tag = 0x10 | (first.len() as u32) << 5;
    out.extend_from_slice(&tag.to_le_bytes()[..3]);
    out.extend_from_slice(&[0x9d, 0x01, 0x2a]);
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(height as u16).to_le_bytes());
    out.extend_from_slice(&first);
    out.extend_from_slice(&second);
    Some(out)
}
