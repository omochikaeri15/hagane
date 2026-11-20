const LUMA: [u16; 3] = [16829, 33038, 6417];
const BLUE: [i32; 3] = [-9714, -19070, 28785];
const RED: [i32; 3] = [28785, -24103, -4681];
const FINE: i32 = 16;
const UNEVEN: u8 = 24;
const MARGIN: u16 = 3;

pub(super) struct Planes {
    pub(super) columns: usize,
    pub(super) rows: usize,
    pub(super) luma: Vec<u8>,
    pub(super) blue: Vec<u8>,
    pub(super) red: Vec<u8>,
    pub(super) peaks: Vec<u8>,
}

struct Tints {
    across: usize,
    down: usize,
    stride: usize,
    first: [Vec<i16>; 2],
    seen: Vec<bool>,
    known: Vec<bool>,
    uneven: Vec<u32>,
    weights: Vec<u32>,
}

fn patch(values: &mut [u8], shown: &[bool], stride: usize, size: usize) {
    let mut total = 0u32;
    let mut count = 0u32;
    let mut empty = [false; 16];
    let blocks = size / 4;
    for block in 0..blocks * blocks {
        let origin = (block / blocks) * 4 * stride + (block % blocks) * 4;
        let mut sum = 0u32;
        let mut live = 0u32;
        for row in 0..4 {
            for column in 0..4 {
                if shown[(block / blocks * 4 + row) * size + block % blocks * 4 + column] {
                    sum += u32::from(values[origin + row * stride + column]);
                    live += 1;
                }
            }
        }
        total += sum;
        count += live;
        empty[block] = live == 0;
        if live > 0 && live < 16 {
            let fill = ((sum + live / 2) / live) as u8;
            for row in 0..4 {
                for column in 0..4 {
                    if !shown[(block / blocks * 4 + row) * size + block % blocks * 4 + column] {
                        values[origin + row * stride + column] = fill;
                    }
                }
            }
        }
    }
    let fill = ((total + count / 2) / count.max(1)) as u8;
    for block in (0..blocks * blocks).filter(|&block| empty[block]) {
        let origin = (block / blocks) * 4 * stride + (block % blocks) * 4;
        for row in 0..4 {
            values[origin + row * stride..origin + row * stride + 4].fill(fill);
        }
    }
}

#[inline]
fn lanes(pixel: &[u8]) -> u64 {
    let packed = u64::from(u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]));
    (packed | packed << 24) & 0x00ff_00ff_00ff_00ff
}

#[inline]
fn tint(matrix: &[i32; 3], sums: [i32; 3], shift: u32) -> i16 {
    (((matrix[0] * sums[0] + matrix[1] * sums[1] + matrix[2] * sums[2] + (1 << (shift - 1))) >> shift) + 128 * FINE) as i16
}

fn gather(rgba: &[u8], width: usize, height: usize, stride: usize, luma: &mut [u8]) -> Tints {
    let (across, down) = (width.div_ceil(2), height.div_ceil(2));
    let span = across + 2;
    let mut first = [vec![0i16; span * (down + 2)], vec![0i16; span * (down + 2)]];
    let mut seen = vec![false; across * down];
    let mut uneven = Vec::new();
    for (shades, line) in luma.chunks_exact_mut(stride).zip(rgba.chunks_exact(width * 4)) {
        for (shade, pixel) in shades.iter_mut().zip(line.as_chunks::<4>().0.iter()) {
            let scaled = |channel: usize| ((u32::from(u16::from(pixel[channel]) << 8) * u32::from(LUMA[channel])) >> 16) as u16;
            let sum = scaled(0) + scaled(1) + scaled(2);
            let dark = pixel[0] | pixel[1] | pixel[2] == 0;
            let bright = pixel[0] & pixel[1] & pixel[2] == 255;
            *shade = (((sum + 128) >> 8) + 16 - if dark { MARGIN } else { 0 } + if bright { MARGIN } else { 0 }) as u8;
        }
    }
    let [first_blue, first_red] = &mut first;
    for cy in 0..down {
        let top = &rgba[cy * 2 * width * 4..(cy * 2 + 1) * width * 4];
        let whole = cy * 2 + 1 < height;
        let bottom = if whole { &rgba[(cy * 2 + 1) * width * 4..(cy * 2 + 2) * width * 4] } else { top };
        let at = (cy + 1) * span + 1;
        let blues = &mut first_blue[at..at + across];
        let reds = &mut first_red[at..at + across];
        let flags = &mut seen[cy * across..(cy + 1) * across];
        let cells = blues.iter_mut().zip(reds).zip(flags).zip(top.as_chunks::<8>().0.iter().zip(bottom.as_chunks::<8>().0.iter()));
        for (cx, (((blue, red), flag), (upper, lower))) in cells.enumerate() {
            let alphas = [upper[3], upper[7], lower[3], lower[7]];
            let high = alphas[0].max(alphas[1]).max(alphas[2]).max(alphas[3]);
            if high == 0 {
                continue;
            }
            *flag = true;
            let low = alphas[0].min(alphas[1]).min(alphas[2]).min(alphas[3]);
            if !whole || low == 0 || high - low > UNEVEN {
                uneven.push((cy * across + cx) as u32);
                continue;
            }
            let sum = lanes(&upper[..4]) + lanes(&upper[4..]) + lanes(&lower[..4]) + lanes(&lower[4..]);
            let sums = [(sum & 0xffff) as i32, (sum >> 32 & 0xffff) as i32, (sum >> 16 & 0xffff) as i32];
            *blue = tint(&BLUE, sums, 14);
            *red = tint(&RED, sums, 14);
        }
        if width % 2 == 1 && (top[width * 4 - 1] != 0 || bottom[width * 4 - 1] != 0) {
            seen[cy * across + across - 1] = true;
            uneven.push((cy * across + across - 1) as u32);
        }
    }
    uneven.sort_unstable();
    let mut weights = Vec::with_capacity(uneven.len());
    for &cell in &uneven {
        let (cx, cy) = (cell as usize % across, cell as usize / across);
        let mut sums = [0u32; 4];
        for y in cy * 2..(cy * 2 + 2).min(height) {
            for x in cx * 2..(cx * 2 + 2).min(width) {
                let pixel = &rgba[(y * width + x) * 4..(y * width + x) * 4 + 4];
                let square = u32::from(pixel[3]) * u32::from(pixel[3]);
                sums[0] += square * u32::from(pixel[0]);
                sums[1] += square * u32::from(pixel[1]);
                sums[2] += square * u32::from(pixel[2]);
                sums[3] += square;
            }
        }
        let scale = 64.0 / sums[3] as f32;
        let color = [(sums[0] as f32 * scale + 0.5) as i32, (sums[1] as f32 * scale + 0.5) as i32, (sums[2] as f32 * scale + 0.5) as i32];
        let at = (cy + 1) * span + cx + 1;
        first_blue[at] = tint(&BLUE, color, 18);
        first_red[at] = tint(&RED, color, 18);
        weights.push(sums[3]);
    }
    let known = seen.clone();
    Tints { across, down, stride: span, first, seen, known, uneven, weights }
}

fn spread(tints: &mut Tints) {
    let (across, down, span) = (tints.across, tints.down, tints.stride);
    let before = tints.known.clone();
    for cy in 0..down {
        for cx in 0..across {
            if before[cy * across + cx] {
                continue;
            }
            let mut count = 0;
            let mut total = [0i32; 2];
            for ny in cy.saturating_sub(1)..(cy + 2).min(down) {
                for nx in cx.saturating_sub(1)..(cx + 2).min(across) {
                    if before[ny * across + nx] {
                        count += 1;
                        total[0] += i32::from(tints.first[0][(ny + 1) * span + nx + 1]);
                        total[1] += i32::from(tints.first[1][(ny + 1) * span + nx + 1]);
                    }
                }
            }
            if count > 0 {
                tints.first[0][(cy + 1) * span + cx + 1] = (total[0] / count) as i16;
                tints.first[1][(cy + 1) * span + cx + 1] = (total[1] / count) as i16;
                tints.known[cy * across + cx] = true;
            }
        }
    }
}

fn frame(plane: &mut [i16], across: usize, down: usize, span: usize) {
    for cy in 1..=down {
        plane[cy * span] = plane[cy * span + 1];
        plane[cy * span + across + 1] = plane[cy * span + across];
    }
    plane.copy_within(span..2 * span, 0);
    plane.copy_within(down * span..(down + 1) * span, (down + 1) * span);
}

fn sharpen(tints: &Tints, alpha: &[u8], width: usize, height: usize, rounds: usize) -> [Vec<i16>; 2] {
    let (across, down, span) = (tints.across, tints.down, tints.stride);
    let mut out = [tints.first[0].clone(), tints.first[1].clone()];
    for (plane, first) in out.iter_mut().zip(&tints.first) {
        frame(plane, across, down, span);
        let mut next = plane.clone();
        for _ in 0..rounds {
            for cy in 0..down {
                let (above, rest) = plane[cy * span..].split_at(span);
                let (here, below) = rest.split_at(span);
                let target = &mut next[(cy + 1) * span + 1..(cy + 1) * span + 1 + across];
                let wanted = &first[(cy + 1) * span + 1..(cy + 1) * span + 1 + across];
                let flags = &tints.seen[cy * across..(cy + 1) * across];
                let rows = above.windows(3).zip(here.windows(3)).zip(below.windows(3));
                for (((out, wanted), flag), ((above, here), below)) in target.iter_mut().zip(wanted).zip(flags).zip(rows) {
                    let cross = i32::from(above[1]) + i32::from(below[1]) + i32::from(here[0]) + i32::from(here[2]);
                    let corners = i32::from(above[0]) + i32::from(above[2]) + i32::from(below[0]) + i32::from(below[2]);
                    let middle = i32::from(here[1]);
                    let blurred = ((middle << 5) + (middle << 2) + (cross << 2) + (cross << 1) + corners + 32) >> 6;
                    let moved = (i32::from(here[1]) + i32::from(*wanted) - blurred).clamp(0, 255 * FINE);
                    *out = if *flag { moved as i16 } else { here[1] };
                }
            }
            for (&cell, &total) in tints.uneven.iter().zip(&tints.weights) {
                let (cx, cy) = (cell as usize % across, cell as usize / across);
                let at = (cy + 1) * span + cx + 1;
                let center = 9 * i32::from(plane[at]);
                let mut sum = 0i64;
                for y in cy * 2..(cy * 2 + 2).min(height) {
                    let far = if y % 2 == 1 { at + span } else { at - span };
                    for x in cx * 2..(cx * 2 + 2).min(width) {
                        let side = if x % 2 == 1 { 1 } else { -1 };
                        let weight = i64::from(alpha[y * width + x]);
                        let blend = (center + 3 * i32::from(plane[at.wrapping_add_signed(side)]) + 3 * i32::from(plane[far]) + i32::from(plane[far.wrapping_add_signed(side)]) + 8) >> 4;
                        sum += weight * weight * i64::from(blend);
                    }
                }
                let blurred = (sum / i64::from(total)) as i32;
                next[at] = (i32::from(plane[at]) + i32::from(first[at]) - blurred).clamp(0, 255 * FINE) as i16;
            }
            std::mem::swap(plane, &mut next);
            frame(plane, across, down, span);
        }
    }
    out
}

pub(super) fn convert(rgba: &[u8], alpha: &[u8], width: usize, height: usize, rounds: usize) -> Planes {
    let columns = width.div_ceil(16);
    let rows = height.div_ceil(16);
    let stride = columns * 16;
    let narrow = columns * 8;
    let mut planes = Planes { columns, rows, luma: vec![0; stride * rows * 16], blue: vec![0; narrow * rows * 8], red: vec![0; narrow * rows * 8], peaks: vec![0; columns * rows] };
    let mut tints = gather(rgba, width, height, stride, &mut planes.luma);
    spread(&mut tints);
    let fine = sharpen(&tints, alpha, width, height, rounds);
    let (across, down, span) = (tints.across, tints.down, tints.stride);
    for (target, source) in [&mut planes.blue, &mut planes.red].into_iter().zip(&fine) {
        for cy in 0..down {
            let from = &source[(cy + 1) * span + 1..(cy + 1) * span + 1 + across];
            for (out, value) in target[cy * narrow..cy * narrow + across].iter_mut().zip(from) {
                *out = ((i32::from(*value) + FINE / 2) >> 4).clamp(0, 255) as u8;
            }
        }
    }
    for row in 0..rows {
        for column in 0..columns {
            let tall = (height - row * 16).min(16);
            let wide = (width - column * 16).min(16);
            let mut lowest = 255u8;
            let mut peak = 0u8;
            for y in 0..tall {
                let start = (row * 16 + y) * width + column * 16;
                for &value in &alpha[start..start + wide] {
                    lowest = lowest.min(value);
                    peak = peak.max(value);
                }
            }
            if peak == 0 {
                continue;
            }
            planes.peaks[row * columns + column] = peak;
            if lowest > 0 && tall == 16 && wide == 16 {
                continue;
            }
            let mut shown = [false; 256];
            for y in 0..tall {
                let start = (row * 16 + y) * width + column * 16;
                for (flag, &value) in shown[y * 16..y * 16 + wide].iter_mut().zip(&alpha[start..start + wide]) {
                    *flag = value != 0;
                }
            }
            patch(&mut planes.luma[row * 16 * stride + column * 16..], &shown, stride, 16);
            let mut tinted = [false; 64];
            for y in 0..8.min(down - row * 8) {
                let start = (row * 8 + y) * across + column * 8;
                let run = (across - column * 8).min(8);
                tinted[y * 8..y * 8 + run].copy_from_slice(&tints.known[start..start + run]);
            }
            let origin = row * 8 * narrow + column * 8;
            patch(&mut planes.blue[origin..], &tinted, narrow, 8);
            patch(&mut planes.red[origin..], &tinted, narrow, 8);
        }
    }
    planes
}
