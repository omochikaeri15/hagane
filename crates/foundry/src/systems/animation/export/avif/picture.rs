const TOLERANCE: f32 = 1.0;
const THIN: f32 = 0.4;
const THICKEST: f32 = 4.0;

pub(super) struct Veil {
    pub factors: Vec<f32>,
    pub across: usize,
    pub mean: f32,
}

pub(super) struct Prepared {
    pub stride: usize,
    pub columns4: usize,
    pub rows4: usize,
    pub color: Vec<Vec<u8>>,
    pub alpha: Vec<Vec<u8>>,
    pub empty: Vec<bool>,
}

fn convert(pixel: &[u8; 4]) -> [u8; 3] {
    let (r, g, b) = (i32::from(pixel[0]), i32::from(pixel[1]), i32::from(pixel[2]));
    [
        ((19595 * r + 38470 * g + 7471 * b + 32768) >> 16) as u8,
        (((-11059 * r - 21709 * g + 32768 * b + 32768) >> 16) + 128).clamp(0, 255) as u8,
        (((32768 * r - 27439 * g - 5329 * b + 32768) >> 16) + 128).clamp(0, 255) as u8,
    ]
}

fn stretch(plane: &mut [u8], stride: usize, width: usize, height: usize) {
    if width < stride {
        for line in plane.chunks_exact_mut(stride).take(height) {
            let last = line[width - 1];
            line[width..].fill(last);
        }
    }
    for y in height..plane.len() / stride {
        plane.copy_within((height - 1) * stride..height * stride, y * stride);
    }
}

pub(super) fn prepare(rgba: &[u8], width: usize, height: usize, identity: bool) -> Prepared {
    let stride = width.div_ceil(8) * 8;
    let tall = height.div_ceil(8) * 8;
    let wide = stride / 8;
    let mut alpha = vec![0u8; stride * tall];
    let mut planes = [vec![0u8; stride * tall], vec![0u8; stride * tall], vec![0u8; stride * tall]];
    for (line, pixels) in alpha.chunks_exact_mut(stride).zip(rgba.chunks_exact(width * 4)) {
        for (value, pixel) in line.iter_mut().zip(pixels.as_chunks::<4>().0) {
            *value = pixel[3];
        }
    }
    stretch(&mut alpha, stride, width, height);
    let mut empty = vec![false; wide * (tall / 8)];
    let [first, second, third] = &mut planes;
    if identity {
        for (y, pixels) in rgba.chunks_exact(width * 4).take(height).enumerate() {
            let lines = first[y * stride..].iter_mut().zip(&mut second[y * stride..]).zip(&mut third[y * stride..]);
            for (((green, blue), red), pixel) in lines.zip(pixels.as_chunks::<4>().0) {
                (*green, *blue, *red) = (pixel[1], pixel[2], pixel[0]);
            }
        }
        planes.iter_mut().for_each(|plane| stretch(plane, stride, width, height));
        return Prepared { stride, columns4: stride / 4, rows4: tall / 4, color: planes.into(), alpha: vec![alpha], empty };
    }
    for (index, hollow) in empty.iter_mut().enumerate() {
        let (x, y) = ((index % wide) * 8, (index / wide) * 8);
        let at = y * stride + x;
        let (mut least, mut most) = (255u8, 0u8);
        for row in 0..8 {
            for &value in &alpha[at + row * stride..at + row * stride + 8] {
                least = least.min(value);
                most = most.max(value);
            }
        }
        *hollow = most == 0;
        if *hollow {
            continue;
        }
        let real = (width - x).min(8);
        for row in 0..8 {
            let from = ((y + row).min(height - 1) * width + x) * 4;
            let to = at + row * stride;
            let mut last = [0u8; 3];
            for (column, pixel) in rgba[from..from + real * 4].as_chunks::<4>().0.iter().enumerate() {
                last = convert(pixel);
                (first[to + column], second[to + column], third[to + column]) = (last[0], last[1], last[2]);
            }
            first[to + real..to + 8].fill(last[0]);
            second[to + real..to + 8].fill(last[1]);
            third[to + real..to + 8].fill(last[2]);
        }
        if least > 0 {
            continue;
        }
        let mut sums = [0u32; 3];
        let mut seen = 0u32;
        for row in 0..8 {
            let to = at + row * stride;
            for column in (0..8).filter(|&column| alpha[to + column] != 0) {
                seen += 1;
                sums[0] += u32::from(first[to + column]);
                sums[1] += u32::from(second[to + column]);
                sums[2] += u32::from(third[to + column]);
            }
        }
        let fill = sums.map(|sum| ((sum + seen / 2) / seen.max(1)) as u8);
        for row in 0..8 {
            let to = at + row * stride;
            for column in (0..8).filter(|&column| alpha[to + column] == 0) {
                (first[to + column], second[to + column], third[to + column]) = (fill[0], fill[1], fill[2]);
            }
        }
    }
    Prepared { stride, columns4: stride / 4, rows4: tall / 4, color: planes.into(), alpha: vec![alpha], empty }
}

fn across(rgba: &[u8], width: usize, y: usize, line: &mut [[u32; 4]], weighted: &mut [[u32; 4]]) {
    for (value, pixel) in weighted.iter_mut().zip(rgba[y * width * 4..(y + 1) * width * 4].as_chunks::<4>().0) {
        let share = u32::from(pixel[3]);
        *value = [u32::from(pixel[0]) * share, u32::from(pixel[1]) * share, u32::from(pixel[2]) * share, share];
    }
    let add = |a: [u32; 4], b: [u32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
    line[0] = if width > 1 { add(weighted[0], weighted[1]) } else { weighted[0] };
    for (sum, near) in line[1..].iter_mut().zip(weighted.windows(3)) {
        *sum = add(add(near[0], near[1]), near[2]);
    }
    if width > 1 {
        line[width - 1] = add(weighted[width - 2], weighted[width - 1]);
    }
}

pub(super) fn soften(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let mut rows = [vec![[0u32; 4]; width], vec![[0u32; 4]; width], vec![[0u32; 4]; width]];
    let mut weighted = vec![[0u32; 4]; width];
    let mut ready = [usize::MAX; 3];
    for y in 0..height {
        let line = &rgba[y * width * 4..(y + 1) * width * 4];
        if !line.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0 && pixel[3] != 255) {
            continue;
        }
        let (first, last) = (y.saturating_sub(1), (y + 1).min(height - 1));
        for row in first..=last {
            if ready[row % 3] != row {
                across(rgba, width, row, &mut rows[row % 3], &mut weighted);
                ready[row % 3] = row;
            }
        }
        for (x, pixel) in line.as_chunks::<4>().0.iter().enumerate() {
            let alpha = pixel[3];
            if alpha == 0 || alpha == 255 {
                continue;
            }
            let mut sums = [0u32; 4];
            for row in first..=last {
                for (sum, value) in sums.iter_mut().zip(rows[row % 3][x]) {
                    *sum += value;
                }
            }
            let weight = sums[3] as f32;
            let reach = TOLERANCE * 255.0 / f32::from(alpha);
            let at = (y * width + x) * 4;
            for (channel, &sum) in sums[..3].iter().enumerate() {
                let value = f32::from(pixel[channel]);
                let kept = (sum as f32 / weight).clamp(value - reach, value + reach).max(0.0);
                let whole = kept as u32;
                out[at + channel] = (whole + u32::from(kept - whole as f32 >= 0.5)).min(255) as u8;
            }
        }
    }
    out
}

pub(super) fn veil(alpha: &[u8], stride: usize, columns4: usize, rows4: usize) -> Veil {
    let (wide, tall) = (columns4 * 4, rows4 * 4);
    let across = wide.div_ceil(64);
    let mut factors = vec![0f32; across * tall.div_ceil(64)];
    let (mut weighted, mut pixels) = (0f32, 0f32);
    for (index, factor) in factors.iter_mut().enumerate() {
        let (left, top) = ((index % across) * 64, (index / across) * 64);
        let (mut total, mut seen) = (0u32, 0u32);
        for y in top..(top + 64).min(tall) {
            for &value in alpha[y * stride + left..y * stride + (left + 64).min(wide)].iter().filter(|&&value| value > 0) {
                total += u32::from(value);
                seen += 1;
            }
        }
        if seen > 0 {
            *factor = (255.0 * seen as f32 / total as f32).powf(THIN).clamp(1.0, THICKEST);
            weighted += seen as f32 * *factor;
            pixels += seen as f32;
        }
    }
    Veil { factors, across, mean: if pixels > 0.0 { weighted / pixels } else { 1.0 } }
}

#[cfg(test)]
mod tests {
    use super::{TOLERANCE, soften};

    #[test]
    fn softening_moves_premultiplied_color_by_at_most_the_tolerance() {
        let (width, height) = (9, 7);
        let mut state = 5u32;
        let rgba: Vec<u8> = (0..width * height * 4)
            .map(|index| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                // Every fourth byte is alpha; keep some pixels fully clear and some fully opaque.
                if index % 4 == 3 { [0, 255, 7, 40, 128, 250][(state >> 16) as usize % 6] } else { (state >> 16) as u8 }
            })
            .collect();
        let out = soften(&rgba, width, height);
        for (old, new) in rgba.as_chunks::<4>().0.iter().zip(out.as_chunks::<4>().0) {
            assert_eq!(old[3], new[3]);
            if old[3] == 0 || old[3] == 255 {
                assert_eq!(old, new);
            }
            for channel in 0..3 {
                let moved = (f32::from(old[channel]) - f32::from(new[channel])).abs() * f32::from(old[3]) / 255.0;
                assert!(moved <= TOLERANCE + 0.5, "moved {moved}");
            }
        }
    }
}
