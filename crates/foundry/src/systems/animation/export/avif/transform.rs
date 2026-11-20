mod kernels;

use std::array;

pub(super) const COSINE: u8 = 0;
pub(super) const SINE_DOWN: u8 = 1;
pub(super) const SINE_ACROSS: u8 = 2;
pub(super) const SINE: u8 = 3;
pub(super) const IDENTITY: u8 = 4;
pub(super) const FLIP_DOWN: u8 = 5;
pub(super) const FLIP_ACROSS: u8 = 6;
pub(super) const FLIP: u8 = 7;
pub(super) const SINE_FLIP: u8 = 8;
pub(super) const FLIP_SINE: u8 = 9;

fn unflip(kind: u8) -> (u8, bool, bool) {
    match kind {
        FLIP_DOWN => (SINE_DOWN, true, false),
        FLIP_ACROSS => (SINE_ACROSS, false, true),
        FLIP => (SINE, true, true),
        SINE_FLIP => (SINE, false, true),
        FLIP_SINE => (SINE, true, false),
        _ => (kind, false, false),
    }
}

fn mirror(block: &mut [i32], size: usize, down: bool, across: bool) {
    if across {
        block.chunks_exact_mut(size).for_each(<[i32]>::reverse);
    }
    if down {
        for row in 0..size / 2 {
            let (top, bottom) = block.split_at_mut((size - 1 - row) * size);
            top[row * size..(row + 1) * size].swap_with_slice(&mut bottom[..size]);
        }
    }
}

const SINE_TAPS: [i32; 4] = [1321, 2482, 3344, 3803];

fn rotate<const L: usize>(a: [i16; L], b: [i16; L], cos: i32, sin: i32) -> ([i16; L], [i16; L]) {
    (
        array::from_fn(|i| ((i32::from(a[i]) * cos - i32::from(b[i]) * sin + 2048) >> 12) as i16),
        array::from_fn(|i| ((i32::from(a[i]) * sin + i32::from(b[i]) * cos + 2048) >> 12) as i16),
    )
}

fn clamped<const L: usize>(a: [i16; L], b: [i16; L]) -> ([i16; L], [i16; L]) {
    (array::from_fn(|i| a[i].saturating_add(b[i])), array::from_fn(|i| a[i].saturating_sub(b[i])))
}

fn spread<const L: usize>(a: [i16; L], b: [i16; L]) -> ([i16; L], [i16; L]) {
    clamped(a, b)
}

fn negate<const L: usize>(a: [i16; L]) -> [i16; L] {
    a.map(i16::saturating_neg)
}

fn inverse_adst4<const L: usize>(t: &mut [[i16; L]; 4]) {
    let [s1, s2, s3, s4] = SINE_TAPS;
    let input = *t;
    for lane in 0..L {
        let [a, b, c, d] = [i32::from(input[0][lane]), i32::from(input[1][lane]), i32::from(input[2][lane]), i32::from(input[3][lane])];
        let first = s1 * a + s4 * c + s2 * d;
        let second = s2 * a - s1 * c - s4 * d;
        let third = s3 * (a - c + d);
        let fourth = s3 * b;
        t[0][lane] = ((first + fourth + 2048) >> 12) as i16;
        t[1][lane] = ((second + fourth + 2048) >> 12) as i16;
        t[2][lane] = ((third + 2048) >> 12) as i16;
        t[3][lane] = ((first + second - fourth + 2048) >> 12) as i16;
    }
}

fn forward_adst4<const L: usize>(t: &mut [[i16; L]; 4]) {
    let [s1, s2, s3, s4] = SINE_TAPS;
    let input = *t;
    for lane in 0..L {
        let [a, b, c, d] = [i32::from(input[0][lane]), i32::from(input[1][lane]), i32::from(input[2][lane]), i32::from(input[3][lane])];
        t[0][lane] = ((s1 * a + s2 * b + s3 * c + s4 * d + 2048) >> 12) as i16;
        t[1][lane] = ((s3 * (a + b - d) + 2048) >> 12) as i16;
        t[2][lane] = ((s4 * a - s1 * b - s3 * c + s2 * d + 2048) >> 12) as i16;
        t[3][lane] = ((s2 * a - s4 * b + s3 * c - s1 * d + 2048) >> 12) as i16;
    }
}

fn flip<const N: usize>(t: &[[i16; N]; N], shift: u32) -> [[i16; N]; N] {
    let round = (1 << shift) >> 1;
    array::from_fn(|i| array::from_fn(|j| ((i32::from(t[j][i]) + round) >> shift).clamp(-32768, 32767) as i16))
}

fn inverse_square<const N: usize>(
    coefs: &[i32],
    out: &mut [i32],
    shift: u32,
    across: impl Fn(&mut [[i16; N]; N]),
    down: impl Fn(&mut [[i16; N]; N]),
) {
    let mut t = [[0i16; N]; N];
    for (i, line) in coefs.as_chunks::<N>().0.iter().enumerate() {
        for (j, &value) in line.iter().enumerate() {
            t[j][i] = value as i16;
        }
    }
    across(&mut t);
    let mut t = flip(&t, shift);
    down(&mut t);
    for (line, row) in out.as_chunks_mut::<N>().0.iter_mut().zip(&t) {
        for (value, &sample) in line.iter_mut().zip(row) {
            *value = (i32::from(sample) + 8) >> 4;
        }
    }
}

fn forward_square<const N: usize>(
    residual: &[i32],
    out: &mut [i32],
    shifts: [u32; 3],
    down: impl Fn(&mut [[i16; N]; N]),
    across: impl Fn(&mut [[i16; N]; N]),
) {
    let mut t = [[0i16; N]; N];
    for (row, line) in t.iter_mut().zip(residual.as_chunks::<N>().0.iter()) {
        for (value, &sample) in row.iter_mut().zip(line) {
            *value = (sample << shifts[0]) as i16;
        }
    }
    down(&mut t);
    let mut t = flip(&t, shifts[1]);
    across(&mut t);
    for (i, line) in out.as_chunks_mut::<N>().0.iter_mut().enumerate() {
        for (j, value) in line.iter_mut().enumerate() {
            *value = i32::from(t[j][i]) << shifts[2];
        }
    }
}

fn turned<const N: usize, const C: usize>(t: &[[[i16; 8]; N]; C], shift: u32) -> [[[i16; 8]; N]; C] {
    let round = (1 << shift) >> 1;
    array::from_fn(|chunk| {
        array::from_fn(|i| array::from_fn(|lane| ((i32::from(t[i / 8][chunk * 8 + lane][i % 8]) + round) >> shift).clamp(-32768, 32767) as i16))
    })
}

fn inverse_wide<const N: usize, const C: usize>(
    coefs: &[i32],
    out: &mut [i32],
    shift: u32,
    across: impl Fn(&mut [[i16; 8]; N]),
    down: impl Fn(&mut [[i16; 8]; N]),
) {
    let mut t = [[[0i16; 8]; N]; C];
    for (i, line) in coefs.as_chunks::<N>().0.iter().enumerate() {
        for (j, &value) in line.iter().enumerate() {
            t[i / 8][j][i % 8] = value as i16;
        }
    }
    t.iter_mut().for_each(&across);
    let mut t = turned(&t, shift);
    t.iter_mut().for_each(&down);
    for (i, line) in out.as_chunks_mut::<N>().0.iter_mut().enumerate() {
        for (j, value) in line.iter_mut().enumerate() {
            *value = (i32::from(t[j / 8][i][j % 8]) + 8) >> 4;
        }
    }
}

fn forward_wide<const N: usize, const C: usize>(
    residual: &[i32],
    out: &mut [i32],
    shifts: [u32; 3],
    down: impl Fn(&mut [[i16; 8]; N]),
    across: impl Fn(&mut [[i16; 8]; N]),
) {
    let mut t = [[[0i16; 8]; N]; C];
    for (i, line) in residual.as_chunks::<N>().0.iter().enumerate() {
        for (j, &sample) in line.iter().enumerate() {
            t[j / 8][i][j % 8] = (sample << shifts[0]) as i16;
        }
    }
    t.iter_mut().for_each(&down);
    let mut t = turned(&t, shifts[1]);
    t.iter_mut().for_each(&across);
    for (i, line) in out.as_chunks_mut::<N>().0.iter_mut().enumerate() {
        for (j, value) in line.iter_mut().enumerate() {
            *value = i32::from(t[i / 8][j][i % 8]) << shifts[2];
        }
    }
}

fn inverse_plain(coefs: &[i32], out: &mut [i32], size: usize) {
    let (gain, down, shift) = match size {
        4 => (5793, 12, 0),
        8 => (2, 0, 1),
        _ => (11586, 12, 2),
    };
    let scale = |value: i32| (value * gain + ((1 << down) >> 1)) >> down;
    let round = (1 << shift) >> 1;
    for (value, &coef) in out.iter_mut().zip(coefs) {
        let across = ((scale(coef) + round) >> shift).clamp(-32768, 32767);
        *value = (scale(across) + 8) >> 4;
    }
}

pub(super) fn inverse(coefs: &[i32], out: &mut [i32], size: usize, kind: u8) {
    let (kind, down, across) = unflip(kind);
    unflipped_inverse(coefs, out, size, kind);
    mirror(&mut out[..size * size], size, down, across);
}

fn unflipped_inverse(coefs: &[i32], out: &mut [i32], size: usize, kind: u8) {
    use kernels::{inverse_adst8 as s8, inverse_adst16 as s16, inverse_dct4 as c4, inverse_dct8 as c8, inverse_dct16 as c16};
    match (size, kind) {
        (_, IDENTITY) => inverse_plain(coefs, out, size),
        (4, COSINE) => inverse_square::<4>(coefs, out, 0, c4, c4),
        (4, SINE_DOWN) => inverse_square::<4>(coefs, out, 0, c4, inverse_adst4),
        (4, SINE_ACROSS) => inverse_square::<4>(coefs, out, 0, inverse_adst4, c4),
        (4, _) => inverse_square::<4>(coefs, out, 0, inverse_adst4, inverse_adst4),
        (8, COSINE) => inverse_wide::<8, 1>(coefs, out, 1, c8, c8),
        (8, SINE_DOWN) => inverse_wide::<8, 1>(coefs, out, 1, c8, s8),
        (8, SINE_ACROSS) => inverse_wide::<8, 1>(coefs, out, 1, s8, c8),
        (8, _) => inverse_wide::<8, 1>(coefs, out, 1, s8, s8),
        (16, COSINE) => inverse_wide::<16, 2>(coefs, out, 2, c16, c16),
        (16, SINE_DOWN) => inverse_wide::<16, 2>(coefs, out, 2, c16, s16),
        (16, SINE_ACROSS) => inverse_wide::<16, 2>(coefs, out, 2, s16, c16),
        (16, _) => inverse_wide::<16, 2>(coefs, out, 2, s16, s16),
        _ => inverse_square::<32>(coefs, out, 2, kernels::inverse_dct32, kernels::inverse_dct32),
    }
}

pub(super) fn forward(residual: &[i32], out: &mut [i32], size: usize, kind: u8) {
    let (kind, down, across) = unflip(kind);
    if !down && !across {
        unflipped_forward(residual, out, size, kind);
        return;
    }
    let mut flipped = [0i32; 256];
    let flipped = &mut flipped[..size * size];
    flipped.copy_from_slice(&residual[..size * size]);
    mirror(flipped, size, down, across);
    unflipped_forward(flipped, out, size, kind);
}

fn unflipped_forward(residual: &[i32], out: &mut [i32], size: usize, kind: u8) {
    use kernels::{forward_adst8 as s8, forward_adst16 as s16, forward_dct4 as c4, forward_dct8 as c8, forward_dct16 as c16};
    match (size, kind) {
        (_, IDENTITY) => out.iter_mut().zip(residual).for_each(|(value, &sample)| *value = sample << 3),
        (4, COSINE) => forward_square::<4>(residual, out, [2, 0, 0], c4, c4),
        (4, SINE_DOWN) => forward_square::<4>(residual, out, [2, 0, 0], forward_adst4, c4),
        (4, SINE_ACROSS) => forward_square::<4>(residual, out, [2, 0, 0], c4, forward_adst4),
        (4, _) => forward_square::<4>(residual, out, [2, 0, 0], forward_adst4, forward_adst4),
        (8, COSINE) => forward_wide::<8, 1>(residual, out, [2, 1, 0], c8, c8),
        (8, SINE_DOWN) => forward_wide::<8, 1>(residual, out, [2, 1, 0], s8, c8),
        (8, SINE_ACROSS) => forward_wide::<8, 1>(residual, out, [2, 1, 0], c8, s8),
        (8, _) => forward_wide::<8, 1>(residual, out, [2, 1, 0], s8, s8),
        (16, COSINE) => forward_wide::<16, 2>(residual, out, [1, 2, 1], c16, c16),
        (16, SINE_DOWN) => forward_wide::<16, 2>(residual, out, [1, 2, 1], s16, c16),
        (16, SINE_ACROSS) => forward_wide::<16, 2>(residual, out, [1, 2, 1], c16, s16),
        (16, _) => forward_wide::<16, 2>(residual, out, [1, 2, 1], s16, s16),
        _ => forward_square::<32>(residual, out, [0, 3, 1], kernels::forward_dct32, kernels::forward_dct32),
    }
}

fn lift(a: i32, b: i32, c: i32, d: i32) -> [i32; 4] {
    let a = a + b;
    let d = d - c;
    let e = (a - d) >> 1;
    let b = e - b;
    let c = e - c;
    [a - c, c, d + b, b]
}

pub(super) fn forward_exact(residual: &[i32; 16]) -> [i32; 16] {
    let mut mid = [0i32; 16];
    for x in 0..4 {
        let column = lift(residual[x], residual[4 + x], residual[8 + x], residual[12 + x]);
        for (y, value) in column.into_iter().enumerate() {
            mid[y * 4 + x] = value;
        }
    }
    let mut out = [0i32; 16];
    for (line, row) in out.as_chunks_mut::<4>().0.iter_mut().zip(mid.as_chunks::<4>().0.iter()) {
        line.copy_from_slice(&lift(row[0], row[1], row[2], row[3]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{COSINE, FLIP, FLIP_ACROSS, FLIP_DOWN, FLIP_SINE, IDENTITY, SINE, SINE_ACROSS, SINE_DOWN, SINE_FLIP, forward, forward_exact, inverse};

    fn unlift(t: [i32; 4]) -> [i32; 4] {
        // The inverse Walsh-Hadamard steps as the specification lists them.
        let (mut a, mut c, mut d, mut b) = (t[0], t[1], t[2], t[3]);
        a += c;
        d -= b;
        let e = (a - d) >> 1;
        b = e - b;
        c = e - c;
        a -= b;
        d += c;
        [a, b, c, d]
    }

    #[test]
    fn exact_transform_inverts_through_the_decoder_steps() {
        let mut state = 99u32;
        for _ in 0..2000 {
            let mut residual = [0i32; 16];
            for value in &mut residual {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                *value = ((state >> 16) % 511) as i32 - 255;
            }
            let levels = forward_exact(&residual);
            let mut mid = [0i32; 16];
            for (line, row) in mid.as_chunks_mut::<4>().0.iter_mut().zip(levels.as_chunks::<4>().0.iter()) {
                line.copy_from_slice(&unlift([row[0], row[1], row[2], row[3]]));
            }
            let mut back = [0i32; 16];
            for x in 0..4 {
                let column = unlift([mid[x], mid[4 + x], mid[8 + x], mid[12 + x]]);
                for y in 0..4 {
                    back[y * 4 + x] = column[y];
                }
            }
            assert_eq!(back, residual);
        }
    }

    #[test]
    fn lossy_transforms_round_trip_within_a_step() {
        let mut state = 5u32;
        for size in [4usize, 8, 16, 32] {
            let kinds: &[u8] = if size == 32 { &[COSINE] } else { &[COSINE, SINE_DOWN, SINE_ACROSS, SINE, IDENTITY, FLIP_DOWN, FLIP_ACROSS, FLIP, SINE_FLIP, FLIP_SINE] };
            for &kind in kinds {
                let mut residual = vec![0i32; size * size];
                for value in &mut residual {
                    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                    *value = ((state >> 16) % 511) as i32 - 255;
                }
                let mut coefs = vec![0i32; size * size];
                forward(&residual, &mut coefs, size, kind);
                let mut back = vec![0i32; size * size];
                inverse(&coefs, &mut back, size, kind);
                let worst = residual.iter().zip(&back).map(|(a, b)| (a - b).abs()).max().unwrap();
                assert!(worst <= if size > 8 { 4 } else { 2 }, "size {size} kind {kind} is off by {worst}");
            }
        }
    }
}
