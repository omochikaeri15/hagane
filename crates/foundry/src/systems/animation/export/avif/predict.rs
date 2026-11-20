use super::tables::{ANGLE_SLOPE, MODE_ANGLE, SMOOTH_WEIGHTS_4, SMOOTH_WEIGHTS_8, SMOOTH_WEIGHTS_16, SMOOTH_WEIGHTS_32, SMOOTH_WEIGHTS_64};

pub(super) const DC: u8 = 0;
pub(super) const VERTICAL: u8 = 1;
pub(super) const HORIZONTAL: u8 = 2;
pub(super) const SMOOTH: u8 = 9;
pub(super) const SMOOTH_DOWN: u8 = 10;
pub(super) const SMOOTH_ACROSS: u8 = 11;
pub(super) const PAETH: u8 = 12;
pub(super) const LEVEL: i8 = 0;

const ANGLED: [(u8, i32); 8] = [(3, 45), (8, 67), (1, 90), (5, 113), (4, 135), (6, 157), (2, 180), (7, 203)];

pub(super) struct Edges {
    pub above: [u8; 129],
    pub left: [u8; 129],
    pub has_above: bool,
    pub has_left: bool,
}

pub(super) struct Reach {
    pub above: bool,
    pub left: bool,
    pub above_right: bool,
    pub below_left: bool,
}

impl Edges {
    pub(super) fn gather(plane: &[u8], stride: usize, x: usize, y: usize, size: usize, reach: &Reach) -> Self {
        let mut above = [0u8; 129];
        let mut left = [0u8; 129];
        let at = y * stride + x;
        let tall = plane.len() / stride;
        match (reach.above, reach.left) {
            (true, _) => {
                let known = (if reach.above_right { 2 * size } else { size }).min(stride - x);
                above[1..=known].copy_from_slice(&plane[at - stride..at - stride + known]);
                let last = above[known];
                above[known + 1..=2 * size].fill(last);
            }
            (false, true) => above[1..=2 * size].fill(plane[at - 1]),
            (false, false) => above[1..=2 * size].fill(127),
        }
        match (reach.left, reach.above) {
            (true, _) => {
                let known = (if reach.below_left { 2 * size } else { size }).min(tall - y);
                for (row, value) in left[1..=known].iter_mut().enumerate() {
                    *value = plane[at + row * stride - 1];
                }
                let last = left[known];
                left[known + 1..=2 * size].fill(last);
            }
            (false, true) => left[1..=2 * size].fill(plane[at - stride]),
            (false, false) => left[1..=2 * size].fill(129),
        }
        let corner = match (reach.above, reach.left) {
            (true, true) => plane[at - stride - 1],
            (true, false) => plane[at - stride],
            (false, true) => plane[at - 1],
            (false, false) => 128,
        };
        above[0] = corner;
        left[0] = corner;
        Self { above, left, has_above: reach.above, has_left: reach.left }
    }
}

fn weights(size: usize) -> &'static [u16] {
    match size {
        4 => &SMOOTH_WEIGHTS_4,
        8 => &SMOOTH_WEIGHTS_8,
        16 => &SMOOTH_WEIGHTS_16,
        32 => &SMOOTH_WEIGHTS_32,
        _ => &SMOOTH_WEIGHTS_64,
    }
}

pub(super) fn is_angled(mode: u8) -> bool {
    (VERTICAL..=8).contains(&mode)
}

pub(super) fn nearest(angle: i32, fine: bool) -> (u8, i8) {
    let angle = if angle < 33 { angle + 180 } else { angle };
    let (mode, base) = ANGLED.iter().copied().min_by_key(|&(_, base)| (angle - base).abs()).unwrap_or((VERTICAL, 90));
    let turn = if fine { ((angle - base + 1).div_euclid(3)).clamp(-3, 3) } else { 0 };
    (mode, turn as i8)
}

pub(super) fn flat(edges: &Edges, size: usize) -> u8 {
    let above: u32 = edges.above[1..=size].iter().map(|&value| u32::from(value)).sum();
    let left: u32 = edges.left[1..=size].iter().map(|&value| u32::from(value)).sum();
    let shift = size.trailing_zeros();
    let value = match (edges.has_above, edges.has_left) {
        (true, true) => (above + left + size as u32) >> (shift + 1),
        (true, false) => (above + (size as u32 >> 1)) >> shift,
        (false, true) => (left + (size as u32 >> 1)) >> shift,
        (false, false) => 128,
    };
    value as u8
}

fn blend(pair: &[u8], shift: i32) -> u8 {
    ((i32::from(pair[0]) * (32 - shift) + i32::from(pair[1]) * shift + 16) >> 5) as u8
}

fn slanted(angle: i32, edges: &Edges, size: usize, out: &mut [u8]) {
    let span = size as i32;
    if angle < 90 {
        let slope = i32::from(ANGLE_SLOPE[angle as usize]);
        let last = 2 * span - 1;
        for (row, line) in out.chunks_exact_mut(size).take(size).enumerate() {
            let offset = (row as i32 + 1) * slope;
            let (start, shift) = (offset >> 6, (offset >> 1) & 31);
            for (column, value) in line.iter_mut().enumerate() {
                let base = start + column as i32;
                *value = if base < last { blend(&edges.above[base as usize + 1..], shift) } else { edges.above[last as usize + 1] };
            }
        }
    } else if angle < 180 {
        let across = i32::from(ANGLE_SLOPE[(180 - angle) as usize]);
        let down = i32::from(ANGLE_SLOPE[(angle - 90) as usize]);
        for (row, line) in out.chunks_exact_mut(size).take(size).enumerate() {
            let row = row as i32;
            for (column, value) in line.iter_mut().enumerate() {
                let column = column as i32;
                let offset = (column << 6) - (row + 1) * across;
                let base = offset >> 6;
                *value = if base >= -1 {
                    blend(&edges.above[(base + 1) as usize..], (offset >> 1) & 31)
                } else {
                    let offset = (row << 6) - (column + 1) * down;
                    blend(&edges.left[((offset >> 6) + 1) as usize..], (offset >> 1) & 31)
                };
            }
        }
    } else {
        let slope = i32::from(ANGLE_SLOPE[(270 - angle) as usize]);
        for (row, line) in out.chunks_exact_mut(size).take(size).enumerate() {
            for (column, value) in line.iter_mut().enumerate() {
                let offset = (column as i32 + 1) * slope;
                let base = (offset >> 6) + row as i32;
                *value = blend(&edges.left[base as usize + 1..], (offset >> 1) & 31);
            }
        }
    }
}

pub(super) fn predict(mode: u8, turn: i8, edges: &Edges, size: usize, out: &mut [u8]) {
    let above = &edges.above[1..=size];
    let left = &edges.left[1..=size];
    let angle = i32::from(MODE_ANGLE[usize::from(mode)]) + 3 * i32::from(turn);
    match mode {
        DC => out[..size * size].fill(flat(edges, size)),
        _ if is_angled(mode) && angle == 90 => out.chunks_exact_mut(size).take(size).for_each(|line| line.copy_from_slice(above)),
        _ if is_angled(mode) && angle == 180 => out.chunks_exact_mut(size).zip(left).for_each(|(line, &value)| line.fill(value)),
        _ if is_angled(mode) => slanted(angle, edges, size, out),
        SMOOTH => {
            let scale = weights(size);
            let (last_left, last_above) = (u32::from(left[size - 1]), u32::from(above[size - 1]));
            for ((line, &side), &down) in out.chunks_exact_mut(size).zip(left).zip(scale) {
                let down = u32::from(down);
                let fixed = (256 - down) * last_left + 256;
                for ((value, &top), &across) in line.iter_mut().zip(above).zip(scale) {
                    let across = u32::from(across);
                    *value = ((down * u32::from(top) + fixed + across * u32::from(side) + (256 - across) * last_above) >> 9) as u8;
                }
            }
        }
        SMOOTH_DOWN => {
            let last = u32::from(left[size - 1]);
            for (line, &down) in out.chunks_exact_mut(size).zip(weights(size)) {
                let down = u32::from(down);
                let fixed = (256 - down) * last + 128;
                for (value, &top) in line.iter_mut().zip(above) {
                    *value = ((down * u32::from(top) + fixed) >> 8) as u8;
                }
            }
        }
        SMOOTH_ACROSS => {
            let scale = weights(size);
            let last = u32::from(above[size - 1]);
            for (line, &side) in out.chunks_exact_mut(size).zip(left) {
                for (value, &across) in line.iter_mut().zip(scale) {
                    let across = u32::from(across);
                    *value = ((across * u32::from(side) + (256 - across) * last + 128) >> 8) as u8;
                }
            }
        }
        _ => {
            let corner = i32::from(edges.above[0]);
            for (line, &side) in out.chunks_exact_mut(size).zip(left) {
                let side = i32::from(side);
                for (value, &top) in line.iter_mut().zip(above) {
                    let top = i32::from(top);
                    let base = top + side - corner;
                    let (to_left, to_top, to_corner) = ((base - side).abs(), (base - top).abs(), (base - corner).abs());
                    *value = if to_left <= to_top && to_left <= to_corner {
                        side as u8
                    } else if to_top <= to_corner {
                        top as u8
                    } else {
                        corner as u8
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Edges, HORIZONTAL, MODE_ANGLE, Reach, VERTICAL, nearest, predict};

    #[test]
    fn nearest_angle_covers_the_whole_fan() {
        assert_eq!(nearest(90, true), (VERTICAL, 0));
        assert_eq!(nearest(96, true), (VERTICAL, 2));
        assert_eq!(nearest(5, true), (HORIZONTAL, 2));
        assert_eq!(nearest(178, true), (HORIZONTAL, -1));
        assert_eq!(nearest(140, false), (4, 0));
        for angle in 0..180 {
            let (mode, turn) = nearest(angle, true);
            let base = i32::from(MODE_ANGLE[usize::from(mode)]) + 3 * i32::from(turn);
            let want = if angle < 33 { angle + 180 } else { angle };
            assert!((base - want).abs() <= 3, "angle {angle} landed on {base}");
        }
    }

    #[test]
    fn diagonal_edge_is_carried_through_the_block() {
        // A picture that is constant along the down-right diagonal is predicted exactly at 135 degrees.
        let stride = 24;
        let mut plane = vec![0u8; stride * 24];
        for y in 0..24 {
            for x in 0..24 {
                plane[y * stride + x] = (((x as i32 - y as i32) * 9 + 120).clamp(0, 255)) as u8;
            }
        }
        let reach = Reach { above: true, left: true, above_right: true, below_left: true };
        let edges = Edges::gather(&plane, stride, 8, 8, 8, &reach);
        let mut out = [0u8; 64];
        predict(4, 0, &edges, 8, &mut out);
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(out[y * 8 + x], plane[(y + 8) * stride + x + 8]);
            }
        }
    }
}
