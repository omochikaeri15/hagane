use std::ops::Range;

use super::{Cell, Source, cdef, deblock, fan};

const SCALES: [f32; 9] = [0.0, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];
const NEAR: [f32; 3] = [0.7, 1.0, 1.4];
const SIDES: [u8; 9] = [0, 0, 1, 1, 2, 2, 2, 2, 3];

pub(in super::super) struct Canvas<'a> {
    pub frame: &'a Source<'a>,
    pub grid: &'a [Cell],
    pub hollow: &'a [bool],
    pub weight: Option<&'a [u8]>,
}

#[derive(Clone, Copy)]
pub(in super::super) struct Filters {
    pub levels: [u8; 4],
    pub ring: Option<[u8; 5]>,
}

#[derive(Clone, Copy)]
pub(in super::super) enum Guide {
    Fixed,
    Reuse([f32; 5]),
    Near([f32; 5]),
    Full,
}

fn error(plane: &[u8], source: &[u8], canvas: &Canvas<'_>) -> u64 {
    let stride = canvas.frame.stride;
    let wide = stride / 8;
    canvas.hollow.iter().enumerate().filter(|&(_, &hollow)| !hollow).map(|(index, _)| square(plane, source, (index / wide) * 8 * stride + (index % wide) * 8, stride, canvas.weight)).sum()
}

fn square(plane: &[u8], source: &[u8], at: usize, stride: usize, weight: Option<&[u8]>) -> u64 {
    (0..8)
        .map(|row| {
            let from = at + row * stride;
            let pairs = plane[from..from + 8].iter().zip(&source[from..from + 8]);
            match weight {
                Some(weight) => pairs.zip(&weight[from..from + 8]).map(|((&a, &b), &w)| (u64::from(a.abs_diff(b)) * u64::from(w)).pow(2)).sum::<u64>(),
                None => pairs.map(|(&a, &b)| u64::from(a.abs_diff(b)).pow(2)).sum::<u64>(),
            }
        })
        .sum()
}

fn scale(base: u8, ratio: f32, cap: u8) -> u8 {
    ((f32::from(base) * ratio).round() as u8).min(cap)
}

fn ratio(chosen: u8, base: u8) -> f32 {
    if base == 0 { 1.0 } else { f32::from(chosen) / f32::from(base) }
}

fn candidates(base: u8, cap: u8, guide: Guide, at: usize) -> Vec<u8> {
    let mut list: Vec<u8> = match guide {
        Guide::Fixed => vec![base],
        Guide::Reuse(lean) => vec![scale(base, lean[at], cap)],
        Guide::Near(lean) => {
            let center = scale(base, lean[at], cap);
            if center == 0 { vec![0, (base / 4).clamp(1, cap.max(1))] } else { NEAR.iter().map(|&near| scale(center, near, cap)).collect() }
        }
        Guide::Full => SCALES.iter().map(|&factor| scale(base, factor, cap)).collect(),
    };
    list.sort_unstable();
    list.dedup();
    list
}

fn best(candidates: &[u8], mut score: impl FnMut(u8) -> u64) -> u8 {
    if let [only] = candidates {
        return *only;
    }
    candidates.iter().map(|&candidate| (score(candidate), candidate)).min().map_or(0, |(_, candidate)| candidate)
}

fn compose(main: u8, shade: u8) -> [u8; 5] {
    [3 + (main / 4).min(3), main, SIDES[usize::from(main.min(8))], shade, SIDES[usize::from(shade.min(8))]]
}

fn ringing(values: [u8; 5]) -> cdef::Ring {
    let [damping, luma, luma_side, chroma, chroma_side] = values;
    let side = |value: u8| i32::from(value) + i32::from(value == 3);
    cdef::Ring { damping: i32::from(damping), luma: [i32::from(luma), side(luma_side)], chroma: [i32::from(chroma), side(chroma_side)] }
}

fn edges(recon: &[Vec<u8>], canvas: &Canvas<'_>, levels: [u8; 4], guide: Guide, threads: usize) -> [u8; 4] {
    let color = recon.len() > 1;
    let mut chosen = levels;
    let picks: Vec<(usize, &mut u8)> = chosen.iter_mut().enumerate().filter(|&(at, _)| at != 1 && (color || at == 0)).collect();
    fan(picks, threads, |(at, value)| {
        let plane = at.saturating_sub(1);
        let mut scratch = Vec::new();
        *value = best(&candidates(levels[at], 63, guide, plane), |level| {
            scratch.clone_from(&recon[plane]);
            deblock::edges(&mut scratch, canvas.frame, canvas.grid, plane > 0, [level, level]);
            error(&scratch, &canvas.frame.planes[plane], canvas)
        });
    });
    chosen[1] = chosen[0];
    if chosen[0] == 0 || !color {
        chosen[2] = 0;
        chosen[3] = 0;
    }
    chosen
}

fn rings(source: &[Vec<u8>], canvas: &Canvas<'_>, spots: &[cdef::Spot], base: [u8; 5], guide: Guide) -> [u8; 5] {
    let frame = canvas.frame;
    let color = source.len() > 1;
    let (base_main, base_shade) = (base[1], base[3]);
    let stride = frame.stride;
    let mut scratch: Vec<Vec<u8>> = source.to_vec();
    let mut score = |planes: Range<usize>, values: [u8; 5]| -> u64 {
        let strength = ringing(values);
        planes
            .map(|plane| {
                let target = &mut scratch[plane];
                for spot in spots {
                    let at = spot.at(stride);
                    for row in 0..8 {
                        target[at + row * stride..at + row * stride + 8].copy_from_slice(&source[plane][at + row * stride..at + row * stride + 8]);
                    }
                }
                cdef::apply(&source[plane], target, frame, spots, &strength, plane > 0);
                spots.iter().map(|spot| square(target, &frame.planes[plane], spot.at(stride), stride, canvas.weight)).sum::<u64>()
            })
            .sum()
    };
    let main = best(&candidates(base_main, 15, guide, 3), |main| score(0..1, compose(main, base_shade)));
    let shade = if color { best(&candidates(base_shade, 15, guide, 4), |shade| score(1..3, compose(main, shade))) } else { base_shade };
    compose(main, shade)
}

pub(in super::super) fn settle(recon: &mut [Vec<u8>], canvas: &Canvas<'_>, base: Filters, guide: Guide, threads: usize) -> (Filters, [f32; 5]) {
    let Canvas { frame, grid, .. } = *canvas;
    let Filters { levels, ring: values } = base;
    let fixed = matches!(guide, Guide::Fixed);
    let chosen = if fixed { levels } else { edges(recon, canvas, levels, guide, threads) };
    deblock::filter(recon, frame, grid, chosen, threads);
    let mut lean = [ratio(chosen[0], levels[0]), ratio(chosen[2], levels[2]), ratio(chosen[3], levels[3]), 1.0, 1.0];
    let Some(base) = values else {
        return (Filters { levels: chosen, ring: None }, lean);
    };
    let source: Vec<Vec<u8>> = recon.to_vec();
    let spots = cdef::spots(&source[0], frame, grid);
    let ring = if fixed { base } else { rings(&source, canvas, &spots, base, guide) };
    lean[3] = ratio(ring[1], base[1]);
    lean[4] = ratio(ring[3], base[3]);
    let strength = ringing(ring);
    fan(recon.iter_mut().zip(&source).enumerate().collect(), threads, |(index, (plane, source))| cdef::apply(source, plane, frame, &spots, &strength, index > 0));
    (Filters { levels: chosen, ring: Some(ring) }, lean)
}
