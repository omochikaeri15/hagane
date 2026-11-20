use super::plane::{average, gradient, half};

const WIDEST: usize = 32;
const SWITCH: u32 = 16 * 6;

#[inline]
fn each<F: Fn(u8, u8, u8, u8) -> u8>(left: u32, top: u32, corner: u32, right: u32, predict: F) -> u32 {
    let (left, top, corner, right) = (left.to_le_bytes(), top.to_le_bytes(), corner.to_le_bytes(), right.to_le_bytes());
    u32::from_le_bytes(std::array::from_fn(|channel| predict(left[channel], top[channel], corner[channel], right[channel])))
}

#[inline]
fn select(left: u32, top: u32, corner: u32) -> u32 {
    let (l, t, c) = (left.to_le_bytes(), top.to_le_bytes(), corner.to_le_bytes());
    let toward_left: u32 = (0..4).map(|channel| u32::from(t[channel].abs_diff(c[channel]))).sum();
    let toward_top: u32 = (0..4).map(|channel| u32::from(l[channel].abs_diff(c[channel]))).sum();
    if toward_left < toward_top { left } else { top }
}

#[inline]
fn minus(pixel: u32, predicted: u32) -> u32 {
    let (pixel, predicted) = (pixel.to_le_bytes(), predicted.to_le_bytes());
    u32::from_le_bytes(std::array::from_fn(|channel| pixel[channel].wrapping_sub(predicted[channel])))
}

#[inline(always)]
fn row<F: Fn(u32, u32, u32, u32) -> u32>(pixels: &[u32], at: usize, width: usize, out: &mut [u32], predict: F) {
    let count = out.len();
    let here = &pixels[at..at + count];
    let left = &pixels[at - 1..at - 1 + count];
    let top = &pixels[at - width..at - width + count];
    let corner = &pixels[at - width - 1..at - width - 1 + count];
    let right = &pixels[at - width + 1..at - width + 1 + count];
    for (((((out, here), left), top), corner), right) in out.iter_mut().zip(here).zip(left).zip(top).zip(corner).zip(right) {
        *out = minus(*here, predict(*left, *top, *corner, *right));
    }
}

fn residual_row(mode: u8, pixels: &[u32], at: usize, width: usize, out: &mut [u32]) {
    match mode {
        0 => row(pixels, at, width, out, |_, _, _, _| 0xff00_0000),
        2 => row(pixels, at, width, out, |_, top, _, _| top),
        3 => row(pixels, at, width, out, |_, _, _, right| right),
        4 => row(pixels, at, width, out, |_, _, corner, _| corner),
        5 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |left, top, _, right| average(average(left, right), top))),
        6 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |left, _, corner, _| average(left, corner))),
        7 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |left, top, _, _| average(left, top))),
        8 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |_, top, corner, _| average(corner, top))),
        9 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |_, top, _, right| average(top, right))),
        10 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |left, top, corner, right| average(average(left, corner), average(top, right)))),
        11 => row(pixels, at, width, out, |left, top, corner, _| select(left, top, corner)),
        12 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |left, top, corner, _| gradient(left, top, corner))),
        13 => row(pixels, at, width, out, |left, top, corner, right| each(left, top, corner, right, |left, top, corner, _| half(left, top, corner))),
        _ => row(pixels, at, width, out, |left, _, _, _| left),
    }
}

struct Costs {
    counts: [[u32; 256]; 4],
    tables: [[u16; 256]; 4],
}

impl Costs {
    fn new() -> Self {
        let mut counts = [[1u32; 256]; 4];
        for channel in &mut counts {
            for (value, count) in channel.iter_mut().enumerate() {
                *count += 256 >> (value as u8 as i8).unsigned_abs().min(8);
            }
        }
        let mut out = Self { counts, tables: [[0; 256]; 4] };
        out.refresh();
        out
    }

    fn refresh(&mut self) {
        for (table, counts) in self.tables.iter_mut().zip(&self.counts) {
            let whole = (counts.iter().sum::<u32>() as f32).log2();
            for (cost, &count) in table.iter_mut().zip(counts) {
                *cost = ((whole - (count as f32).log2()) * 16.0) as u16;
            }
        }
    }

    #[inline]
    fn weigh(&self, residual: &[u32]) -> u32 {
        residual.iter().map(|&pixel| {
            let bytes = pixel.to_le_bytes();
            u32::from(self.tables[0][usize::from(bytes[0])]) + u32::from(self.tables[1][usize::from(bytes[1])]) + u32::from(self.tables[2][usize::from(bytes[2])]) + u32::from(self.tables[3][usize::from(bytes[3])])
        }).sum()
    }

    fn add(&mut self, residual: &[u32]) {
        for &pixel in residual {
            for (counts, byte) in self.counts.iter_mut().zip(pixel.to_le_bytes()) {
                counts[usize::from(byte)] += 1;
            }
        }
    }
}

pub(super) fn choose(pixels: &[u32], width: usize, height: usize, bits: u32, candidates: &[u8]) -> Vec<u8> {
    let size = 1usize << bits;
    let columns = width.div_ceil(size);
    let rows = height.div_ceil(size);
    let mut modes = vec![0u8; columns * rows];
    let mut previous = candidates.first().copied().unwrap_or(1);
    let mut scratch = [0u32; WIDEST];
    let mut costs = Costs::new();
    for band in 0..rows {
        let top = (band * size).max(1);
        let bottom = ((band + 1) * size).min(height);
        for column in 0..columns {
            let left = (column * size).max(1);
            let right = ((column + 1) * size).min(width);
            if left < right && top < bottom {
                let first = pixels[(top - 1) * width + left - 1];
                let flat = (top - 1..bottom).all(|y| pixels[y * width + left - 1..y * width + right].iter().all(|&value| value == first));
                if !flat {
                    let wide = right - left;
                    let mut best = u32::MAX;
                    for &mode in candidates {
                        let mut cost = if mode != previous { SWITCH } else { 0 };
                        for y in top..bottom {
                            let out = &mut scratch[..wide];
                            residual_row(mode, pixels, y * width + left, width, out);
                            cost += costs.weigh(out);
                            if cost >= best {
                                break;
                            }
                        }
                        if cost < best {
                            best = cost;
                            previous = mode;
                        }
                    }
                    for y in top..bottom {
                        let out = &mut scratch[..wide];
                        residual_row(previous, pixels, y * width + left, width, out);
                        costs.add(out);
                    }
                }
            }
            modes[band * columns + column] = previous;
        }
        costs.refresh();
    }
    modes
}

pub(super) fn residuals(pixels: &[u32], width: usize, height: usize, bits: u32, modes: &[u8]) -> Vec<u32> {
    let block = 1usize << bits;
    let columns = width.div_ceil(block);
    let mut out = vec![0u32; pixels.len()];
    out[0] = minus(pixels[0], 0xff00_0000);
    for x in 1..width {
        out[x] = minus(pixels[x], pixels[x - 1]);
    }
    for y in 1..height {
        let target = &mut out[y * width..(y + 1) * width];
        target[0] = minus(pixels[y * width], pixels[(y - 1) * width]);
        for column in 0..columns {
            let left = (column * block).max(1);
            let right = ((column + 1) * block).min(width);
            if left < right {
                residual_row(modes[(y >> bits) * columns + column], pixels, y * width + left, width, &mut target[left..right]);
            }
        }
    }
    out
}
