pub(super) const FLAT: usize = 0;
pub(super) const VERTICAL: usize = 1;
pub(super) const HORIZONTAL: usize = 2;
pub(super) const MOTION: usize = 3;
pub(super) const SPLIT: usize = 4;
pub(super) const ALIKE: [u8; 4] = [0, 2, 3, 1];

pub(super) struct Edge<'a> {
    pub(super) top: &'a [u8],
    pub(super) left: &'a [u8],
    pub(super) corner: u8,
    pub(super) has_top: bool,
    pub(super) has_left: bool,
}

#[inline]
fn pair(a: u8, b: u8) -> u8 {
    ((u16::from(a) + u16::from(b) + 1) >> 1) as u8
}

#[inline]
fn triple(a: u8, b: u8, c: u8) -> u8 {
    ((u16::from(a) + 2 * u16::from(b) + u16::from(c) + 2) >> 2) as u8
}

pub(super) fn large<const SIZE: usize>(mode: usize, edge: &Edge<'_>, out: &mut [u8]) {
    let Some(top) = edge.top.first_chunk::<SIZE>() else {
        return;
    };
    let shift = SIZE.trailing_zeros();
    match mode {
        VERTICAL => {
            for row in out.as_chunks_mut::<SIZE>().0 {
                row.copy_from_slice(top);
            }
        }
        HORIZONTAL => {
            for (row, left) in out.as_chunks_mut::<SIZE>().0.iter_mut().zip(edge.left) {
                row.fill(*left);
            }
        }
        MOTION => {
            let corner = i16::from(edge.corner);
            for (row, left) in out.as_chunks_mut::<SIZE>().0.iter_mut().zip(edge.left) {
                let base = i16::from(*left) - corner;
                for (pixel, above) in row.iter_mut().zip(top) {
                    *pixel = (base + i16::from(*above)).clamp(0, 255) as u8;
                }
            }
        }
        _ => {
            let above: u32 = top.iter().map(|&value| u32::from(value)).sum();
            let beside: u32 = edge.left[..SIZE].iter().map(|&value| u32::from(value)).sum();
            let value = match (edge.has_top, edge.has_left) {
                (true, true) => (above + beside + SIZE as u32) >> (shift + 1),
                (true, false) => (above + (SIZE as u32 >> 1)) >> shift,
                (false, true) => (beside + (SIZE as u32 >> 1)) >> shift,
                (false, false) => 128,
            };
            out[..SIZE * SIZE].fill(value as u8);
        }
    }
}

#[inline]
pub(super) fn plain(top: &[u8; 8], left: &[u8; 4], corner: u8) -> [[u8; 16]; 4] {
    let mut out = [[0u8; 16]; 4];
    let [a0, a1, a2, a3, a4, ..] = *top;
    let [l0, l1, l2, l3] = *left;
    let p = corner;
    let mean = (u16::from(a0) + u16::from(a1) + u16::from(a2) + u16::from(a3) + u16::from(l0) + u16::from(l1) + u16::from(l2) + u16::from(l3) + 4) >> 3;
    out[0] = [mean as u8; 16];
    for (y, side) in left.iter().enumerate() {
        let base = i16::from(*side) - i16::from(p);
        for x in 0..4 {
            out[1][y * 4 + x] = (base + i16::from(top[x])).clamp(0, 255) as u8;
        }
    }
    let smooth = [triple(p, a0, a1), triple(a0, a1, a2), triple(a1, a2, a3), triple(a2, a3, a4)];
    for row in out[2].as_chunks_mut::<4>().0 {
        row.copy_from_slice(&smooth);
    }
    let sides = [triple(p, l0, l1), triple(l0, l1, l2), triple(l1, l2, l3), triple(l2, l3, l3)];
    for (row, side) in out[3].as_chunks_mut::<4>().0.iter_mut().zip(sides) {
        row.fill(side);
    }
    out
}

#[inline]
pub(super) fn slanted(top: &[u8; 8], left: &[u8; 4], corner: u8) -> [[u8; 16]; 6] {
    let [a0, a1, a2, a3, a4, a5, a6, a7] = *top;
    let [l0, l1, l2, l3] = *left;
    let p = corner;
    let down = [triple(a0, a1, a2), triple(a1, a2, a3), triple(a2, a3, a4), triple(a3, a4, a5), triple(a4, a5, a6), triple(a5, a6, a7), triple(a6, a7, a7)];
    let right = [triple(l0, p, a0), triple(p, a0, a1), triple(a0, a1, a2), triple(a1, a2, a3), triple(l1, l0, p), triple(l2, l1, l0), triple(l3, l2, l1)];
    let steep = [pair(p, a0), pair(a0, a1), pair(a1, a2), pair(a2, a3), right[0], right[1], right[2], right[3], right[4], right[5]];
    let lean = [pair(a0, a1), pair(a1, a2), pair(a2, a3), pair(a3, a4), down[0], down[1], down[2], down[3], down[4], down[5]];
    let low = [pair(l0, p), right[0], right[1], right[2], pair(l1, l0), right[4], pair(l2, l1), right[5], pair(l3, l2), right[6]];
    let up = [pair(l0, l1), triple(l0, l1, l2), pair(l1, l2), triple(l1, l2, l3), pair(l2, l3), triple(l2, l3, l3), l3];
    [
        [down[0], down[1], down[2], down[3], down[1], down[2], down[3], down[4], down[2], down[3], down[4], down[5], down[3], down[4], down[5], down[6]],
        [right[0], right[1], right[2], right[3], right[4], right[0], right[1], right[2], right[5], right[4], right[0], right[1], right[6], right[5], right[4], right[0]],
        [steep[0], steep[1], steep[2], steep[3], steep[4], steep[5], steep[6], steep[7], steep[8], steep[0], steep[1], steep[2], steep[9], steep[4], steep[5], steep[6]],
        [lean[0], lean[1], lean[2], lean[3], lean[4], lean[5], lean[6], lean[7], lean[1], lean[2], lean[3], lean[8], lean[5], lean[6], lean[7], lean[9]],
        [low[0], low[1], low[2], low[3], low[4], low[5], low[0], low[1], low[6], low[7], low[4], low[5], low[8], low[9], low[6], low[7]],
        [up[0], up[1], up[2], up[3], up[2], up[3], up[4], up[5], up[4], up[5], up[6], up[6], up[6], up[6], up[6], up[6]],
    ]
}
