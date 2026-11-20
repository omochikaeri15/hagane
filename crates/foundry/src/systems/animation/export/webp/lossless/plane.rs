const WIDEST: usize = 32;

#[inline]
pub(super) fn average(a: u8, b: u8) -> u8 {
    ((u16::from(a) + u16::from(b)) >> 1) as u8
}

#[inline]
pub(super) fn gradient(left: u8, top: u8, corner: u8) -> u8 {
    (i16::from(left) + i16::from(top) - i16::from(corner)).clamp(0, 255) as u8
}

#[inline]
pub(super) fn half(left: u8, top: u8, corner: u8) -> u8 {
    let mean = i16::from(average(left, top));
    (mean + (mean - i16::from(corner)) / 2).clamp(0, 255) as u8
}

#[inline]
fn select(left: u8, top: u8, corner: u8) -> u8 {
    if top.abs_diff(corner) < left.abs_diff(corner) { left } else { top }
}

#[inline(always)]
fn row<F: Fn(u8, u8, u8, u8) -> u8>(plane: &[u8], at: usize, width: usize, out: &mut [u8], predict: F) {
    let count = out.len();
    let here = &plane[at..at + count];
    let left = &plane[at - 1..at - 1 + count];
    let top = &plane[at - width..at - width + count];
    let corner = &plane[at - width - 1..at - width - 1 + count];
    let right = &plane[at - width + 1..at - width + 1 + count];
    for (((((out, here), left), top), corner), right) in out.iter_mut().zip(here).zip(left).zip(top).zip(corner).zip(right) {
        *out = here.wrapping_sub(predict(*left, *top, *corner, *right));
    }
}

pub(super) fn residual_row(mode: u8, plane: &[u8], at: usize, width: usize, out: &mut [u8]) {
    match mode {
        0 => row(plane, at, width, out, |_, _, _, _| 0),
        2 => row(plane, at, width, out, |_, top, _, _| top),
        3 => row(plane, at, width, out, |_, _, _, right| right),
        4 => row(plane, at, width, out, |_, _, corner, _| corner),
        5 => row(plane, at, width, out, |left, top, _, right| average(average(left, right), top)),
        6 => row(plane, at, width, out, |left, _, corner, _| average(left, corner)),
        7 => row(plane, at, width, out, |left, top, _, _| average(left, top)),
        8 => row(plane, at, width, out, |_, top, corner, _| average(corner, top)),
        9 => row(plane, at, width, out, |_, top, _, right| average(top, right)),
        10 => row(plane, at, width, out, |left, top, corner, right| average(average(left, corner), average(top, right))),
        11 => row(plane, at, width, out, |left, top, corner, _| select(left, top, corner)),
        12 => row(plane, at, width, out, |left, top, corner, _| gradient(left, top, corner)),
        13 => row(plane, at, width, out, |left, top, corner, _| half(left, top, corner)),
        _ => row(plane, at, width, out, |left, _, _, _| left),
    }
}

struct Costs {
    counts: [u32; 256],
    table: [u16; 256],
}

impl Costs {
    fn new() -> Self {
        let mut counts = [1u32; 256];
        for (value, count) in counts.iter_mut().enumerate() {
            let size = (value as u8 as i8).unsigned_abs();
            *count += 256 >> size.min(8);
        }
        let mut out = Self { counts, table: [0; 256] };
        out.refresh();
        out
    }

    fn refresh(&mut self) {
        let total: u32 = self.counts.iter().sum();
        let whole = (total as f32).log2();
        for (cost, &count) in self.table.iter_mut().zip(&self.counts) {
            *cost = ((whole - (count as f32).log2()) * 16.0) as u16;
        }
    }
}

fn weigh(residual: &[u8], costs: Option<&Costs>) -> u32 {
    costs.map_or_else(|| residual.iter().map(|&value| u32::from((value as i8).unsigned_abs())).sum(), |costs| residual.iter().map(|&value| u32::from(costs.table[usize::from(value)])).sum())
}

pub(super) fn choose(plane: &[u8], width: usize, height: usize, bits: u32, candidates: &[u8], entropy: bool) -> Vec<u8> {
    let size = 1usize << bits;
    let columns = width.div_ceil(size);
    let rows = height.div_ceil(size);
    let mut modes = vec![0u8; columns * rows];
    let mut previous = candidates.first().copied().unwrap_or(1);
    let mut scratch = [0u8; WIDEST];
    let mut kept = [0u8; WIDEST * WIDEST];
    let mut costs = entropy.then(Costs::new);
    let switch = if entropy { 16 * 6 } else { 1 };
    for band in 0..rows {
        let top = (band * size).max(1);
        let bottom = ((band + 1) * size).min(height);
        for column in 0..columns {
            let left = (column * size).max(1);
            let right = ((column + 1) * size).min(width);
            if left < right && top < bottom {
                let first = plane[(top - 1) * width + left - 1];
                let flat = (top - 1..bottom).all(|y| plane[y * width + left - 1..y * width + right].iter().all(|&value| value == first));
                if !flat {
                    let mut best = u32::MAX;
                    let wide = right - left;
                    for &mode in candidates {
                        let mut cost = if mode != previous { switch } else { 0 };
                        for y in top..bottom {
                            let out = &mut scratch[..wide];
                            residual_row(mode, plane, y * width + left, width, out);
                            cost += weigh(out, costs.as_ref());
                            if cost >= best {
                                break;
                            }
                        }
                        if cost < best {
                            best = cost;
                            previous = mode;
                        }
                    }
                    if let Some(costs) = costs.as_mut() {
                        for y in top..bottom {
                            let out = &mut kept[..wide];
                            residual_row(previous, plane, y * width + left, width, out);
                            for &value in out.iter() {
                                costs.counts[usize::from(value)] += 1;
                            }
                        }
                    }
                }
            }
            modes[band * columns + column] = previous;
        }
        if let Some(costs) = costs.as_mut() {
            costs.refresh();
        }
    }
    modes
}

pub(super) fn residuals(plane: &[u8], width: usize, height: usize, bits: u32, modes: &[u8]) -> Vec<u8> {
    let size = 1usize << bits;
    let columns = width.div_ceil(size);
    let mut out = vec![0u8; plane.len()];
    out[0] = plane[0];
    for x in 1..width {
        out[x] = plane[x].wrapping_sub(plane[x - 1]);
    }
    for y in 1..height {
        let target = &mut out[y * width..(y + 1) * width];
        target[0] = plane[y * width].wrapping_sub(plane[(y - 1) * width]);
        for column in 0..columns {
            let left = (column * size).max(1);
            let right = ((column + 1) * size).min(width);
            if left < right {
                residual_row(modes[(y >> bits) * columns + column], plane, y * width + left, width, &mut target[left..right]);
            }
        }
    }
    out
}

pub(super) fn classify(residual: &[u8], width: usize, height: usize, bits: u32, bounds: &[u32; 3]) -> (Vec<u8>, usize) {
    let size = 1usize << bits;
    let columns = width.div_ceil(size);
    let rows = height.div_ceil(size);
    let mut classes = vec![0u8; columns * rows];
    let mut used = [false; 4];
    for band in 0..rows {
        let mut sums = vec![0u32; columns];
        let lines = (band * size..((band + 1) * size).min(height)).len() as u32;
        for y in band * size..((band + 1) * size).min(height) {
            for (sum, chunk) in sums.iter_mut().zip(residual[y * width..(y + 1) * width].chunks(size)) {
                *sum += weigh(chunk, None);
            }
        }
        for (column, sum) in sums.iter().enumerate() {
            let wide = (width - column * size).min(size) as u32;
            let level = sum * 16 / (lines * wide);
            let class = bounds.iter().filter(|&&bound| level >= bound).count();
            classes[band * columns + column] = class as u8;
            used[class] = true;
        }
    }
    let mut dense = [0u8; 4];
    let mut count = 0;
    for class in 0..4 {
        dense[class] = count as u8;
        count += usize::from(used[class]);
    }
    for class in &mut classes {
        *class = dense[usize::from(*class)];
    }
    (classes, count)
}
